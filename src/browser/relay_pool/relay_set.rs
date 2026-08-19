//! The worker's socket pool and the command handler that drives it.
//!
//! Extracted from the reactor loop so that a command has ONE implementation no
//! matter where it arrived from. Commands reach the worker over the reactor
//! bridge or through the shared ring, and both call
//! [`RelaySet::apply`] — so the two transports cannot drift apart in
//! behaviour, only in how the bytes crossed.

use futures::channel::mpsc;
use nostro2::{NostrClientEvent, NostrSubscription};

use super::ingest::NoteIngestor;
use super::websocket::ReadyState;
use super::worker::{Internal, RelayCommand, WorkerSocket};

/// Every relay socket the worker owns, plus the filter state they feed.
pub struct RelaySet {
    sockets: Vec<WorkerSocket>,
    ingestor: NoteIngestor,
    bus_tx: mpsc::UnboundedSender<Internal>,
}

impl RelaySet {
    #[must_use]
    pub const fn new(ingestor: NoteIngestor, bus_tx: mpsc::UnboundedSender<Internal>) -> Self {
        Self {
            sockets: Vec::new(),
            ingestor,
            bus_tx,
        }
    }

    /// Run one command against the pool.
    pub fn apply(&mut self, command: RelayCommand) {
        match command {
            RelayCommand::Connect(urls) => {
                for url in urls {
                    self.connect_if_new(&url);
                }
                self.emit_health();
            }
            RelayCommand::AddRelay(url) => {
                if self.connect_if_new(&url) {
                    self.emit_health();
                }
            }
            RelayCommand::RemoveRelay(url) => {
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
                self.broadcast(&req_json);
            }
            RelayCommand::Close(sub_id) => {
                self.ingestor.remove_filter(&sub_id);
                let close = NostrClientEvent::close_subscription(&sub_id);
                if let Ok(close_str) = crate::NostrJson::to_string(&close) {
                    self.broadcast(&close_str);
                }
            }
            RelayCommand::Send(event_json) => self.broadcast(&event_json),
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
            self.bus_tx.clone(),
        ) {
            self.sockets.push(sock);
        }
    }

    fn broadcast(&self, frame: &str) {
        for sock in &self.sockets {
            sock.send_str(frame);
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
