//! The worker's socket pool and the command handler that drives it.
//!
//! Extracted from the reactor loop so that a command has ONE implementation no
//! matter where it arrived from. Commands reach the worker over the reactor
//! bridge or through the shared ring, and both call
//! [`RelaySet::apply`] — so the two transports cannot drift apart in
//! behaviour, only in how the bytes crossed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use futures::channel::mpsc;
use nostro2::{NostrClientEvent, NostrSubscription};

use super::ingest::NoteIngestor;
use super::nostr_relay::UserRelay;
use super::websocket::ReadyState;
use super::worker::{Internal, RelayCommand, WorkerSocket};

/// Which traffic each relay takes, keyed by URL.
///
/// Shared with every socket, because a socket replays the active
/// subscriptions when it opens and has to know whether its relay reads. A
/// relay with no entry takes everything.
#[derive(Clone, Default)]
pub struct RelayPolicies(Rc<RefCell<HashMap<String, (bool, bool)>>>);

impl RelayPolicies {
    fn set(&self, relay: &UserRelay) {
        self.0
            .borrow_mut()
            .insert(relay.url.clone(), (relay.read, relay.write));
    }

    fn forget(&self, url: &str) {
        self.0.borrow_mut().remove(url);
    }

    /// Whether subscriptions (REQ / CLOSE) go to this relay.
    #[must_use]
    pub fn reads(&self, url: &str) -> bool {
        self.0.borrow().get(url).is_none_or(|(read, _)| *read)
    }

    /// Whether published events go to this relay.
    #[must_use]
    pub fn writes(&self, url: &str) -> bool {
        self.0.borrow().get(url).is_none_or(|(_, write)| *write)
    }
}

/// Which relays a client frame is meant for.
#[derive(Clone, Copy)]
enum Audience {
    Readers,
    Writers,
    Everyone,
}

impl Audience {
    /// Decide by the frame's own label, the first element of its JSON array.
    ///
    /// `Send` carries any client frame, not only a publish. An EVENT goes to
    /// the relays that write; REQ, CLOSE and COUNT go to the relays that
    /// read; anything else (AUTH in particular, which a read-only relay may
    /// demand too) goes to every relay.
    fn of(frame: &str) -> Self {
        let label = Self::label(frame);
        if label == "EVENT" {
            Self::Writers
        } else if ["REQ", "CLOSE", "COUNT"].contains(&label) {
            Self::Readers
        } else {
            Self::Everyone
        }
    }

    /// `EVENT` in `["EVENT", …]`. Empty when the frame does not start with a
    /// JSON string.
    fn label(frame: &str) -> &str {
        frame
            .trim_start()
            .strip_prefix('[')
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('"'))
            .and_then(|rest| rest.split('"').next())
            .unwrap_or("")
    }

    fn includes(self, policies: &RelayPolicies, url: &str) -> bool {
        match self {
            Self::Readers => policies.reads(url),
            Self::Writers => policies.writes(url),
            Self::Everyone => true,
        }
    }
}

/// Every relay socket the worker owns, plus the filter state they feed.
pub struct RelaySet {
    sockets: Vec<WorkerSocket>,
    policies: RelayPolicies,
    ingestor: NoteIngestor,
    bus_tx: mpsc::UnboundedSender<Internal>,
}

impl RelaySet {
    #[must_use]
    pub fn new(ingestor: NoteIngestor, bus_tx: mpsc::UnboundedSender<Internal>) -> Self {
        Self {
            sockets: Vec::new(),
            policies: RelayPolicies::default(),
            ingestor,
            bus_tx,
        }
    }

    /// Run one command against the pool.
    pub fn apply(&mut self, command: RelayCommand) {
        match command {
            RelayCommand::Connect(relays) => {
                for relay in relays {
                    self.admit(&relay);
                }
                self.emit_health();
            }
            RelayCommand::AddRelay(relay) => {
                if self.admit(&relay) {
                    self.emit_health();
                }
            }
            RelayCommand::RemoveRelay(url) => {
                self.policies.forget(&url);
                self.sockets.retain(|s| s.url() != url);
                self.emit_health();
            }
            RelayCommand::Subscribe {
                sub_id,
                filter_json,
                req_json,
            } => {
                if let Ok(filter) = crate::NostrJson::parse_str::<NostrSubscription>(&filter_json) {
                    self.ingestor.insert_filter(sub_id, filter);
                }
                self.broadcast(Audience::Readers, &req_json);
            }
            RelayCommand::Close(sub_id) => {
                self.ingestor.remove_filter(&sub_id);
                let close = NostrClientEvent::close_subscription(&sub_id);
                if let Ok(close_str) = crate::NostrJson::to_string(&close) {
                    self.broadcast(Audience::Readers, &close_str);
                }
            }
            RelayCommand::Send(frame) => self.broadcast(Audience::of(&frame), &frame),
            #[cfg(feature = "bench-harness")]
            RelayCommand::Flood(spec) => {
                let bus = self.bus_tx.clone();
                super::flood::FloodRunner::new(self.ingestor.clone()).run(spec, move |out| {
                    bus.unbounded_send(Internal::Out(out)).is_ok()
                });
            }
        }
    }

    /// Replace a dead socket, keeping exactly one socket per URL.
    pub fn reconnect(&mut self, url: &str, retry_count: u32) {
        self.sockets.retain(|s| s.url() != url);
        self.connect(url, retry_count);
        self.emit_health();
    }

    /// Record what a relay takes, then connect it unless it already is.
    /// Returns whether a socket was added.
    ///
    /// A connected relay whose `read` flag changes gets a fresh socket. Its
    /// open subscriptions were set up under the old flag: a relay that stops
    /// reading would keep streaming them (CLOSE no longer reaches it), and one
    /// that starts reading would hear nothing until its next reconnect. The
    /// new socket replays the subscriptions by the new flag when it opens.
    fn admit(&mut self, relay: &UserRelay) -> bool {
        if self.policies.reads(&relay.url) != relay.read {
            self.sockets.retain(|s| s.url() != relay.url);
        }
        self.policies.set(relay);
        self.connect_if_new(&relay.url)
    }

    fn connect_if_new(&mut self, url: &str) -> bool {
        if self.sockets.iter().any(|s| s.url() == url) {
            return false;
        }
        self.connect(url, 0);
        true
    }

    fn connect(&mut self, url: &str, retry_count: u32) {
        if let Ok(sock) = WorkerSocket::connect(
            url.to_string(),
            retry_count,
            self.ingestor.clone(),
            self.policies.clone(),
            self.bus_tx.clone(),
        ) {
            self.sockets.push(sock);
        }
    }

    /// Send a frame to the sockets whose relay is in `audience`.
    fn broadcast(&self, audience: Audience, frame: &str) {
        for sock in &self.sockets {
            if audience.includes(&self.policies, sock.url()) {
                sock.send_str(frame);
            }
        }
    }

    /// Snapshot every socket's ready state and ship it as a health update.
    pub fn emit_health(&self) {
        let health = self
            .sockets
            .iter()
            .map(|s| (s.url().to_string(), s.ready_state()))
            .collect::<Vec<(String, ReadyState)>>();
        let _ = self.bus_tx.unbounded_send(Internal::Health(health));
    }
}
