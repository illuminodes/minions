//! The relay reactor: a `yew-agent` Reactor that runs in a Web Worker.
//!
//! It owns the WebSocket pool, the bounded dedup tracker, and performs all
//! client-side filter matching — every bit of relay I/O happens OFF the UI
//! thread. Matched notes and relay events stream back to the main thread over
//! the reactor bridge (JSON-coded `postMessage`).
//!
//! The trade is explicit: we move JSON parsing + dedup + filter-matching off the
//! UI thread, paying a `postMessage` hop per note crossing the worker boundary.
//! Relay frames cross as their original strings in both directions, so the hop
//! adds no re-encoding — only the main thread's own parse. The `worker_bench/`
//! experiment validated that this is a net win under load — this module is the
//! proven reactor body lifted into the library (minus the bench plumbing).
//!
//! The matching the worker does is authoritative for *whether a note is worth
//! shipping at all*; the main thread re-checks `note_matches_filter` only to
//! ROUTE each shipped note to the right per-subscription callback (see
//! `provider::dispatch_note`).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew_agent::prelude::*;

use super::ingest::NoteIngestor;
use super::ingested::Ingested;
use super::relay_set::RelaySet;
use super::worker_sink::WorkerSink;
use super::ReadyState;

const MAX_RECONNECT_SECS: u32 = 120;
const MAX_RETRIES: u32 = 10;

/// JSON codec for the worker bridge.
///
/// `yew-agent`'s default `Bincode` codec panics on the wire frames
/// (`SequenceMustHaveLength`) — bincode cannot encode them without an up-front
/// length. JSON has no such requirement, so both the provider (spawner) and
/// the registrar use this codec. (JSON is also exactly the relay wire format,
/// so nothing extra is paid in translation.)
pub struct JsonCodec;

impl yew_agent::Codec for JsonCodec {
    fn encode<I>(input: I) -> JsValue
    where
        I: Serialize,
    {
        let s = serde_json::to_string(&input).expect("worker message: serialize");
        JsValue::from_str(&s)
    }

    fn decode<O>(input: JsValue) -> O
    where
        O: for<'de> Deserialize<'de>,
    {
        let s = input.as_string().unwrap_or_else(|| {
            // The ring handshake is the only non-string frame on this wire, and
            // `decode` is the only place the application sees a raw `JsValue`
            // from the worker (`yew-agent` owns the single `onmessage`). It is
            // claimed here and reported as a message the reactor ignores.
            assert!(
                super::handshake::RingHandshake::claim(&input),
                "worker message: expected string"
            );
            super::handshake::RingHandshake::DECODES_AS.to_string()
        });
        serde_json::from_str(&s).expect("worker message: deserialize")
    }
}

/// Commands the application (main thread) sends INTO the worker.
///
/// Every Nostr payload crosses as a JSON **string** in relay wire format, not
/// as a typed `nostro2` value. The bridge frames therefore need only `serde`,
/// while the Nostr types keep using whichever JSON backend the build picked
/// (`serde_json` or `json-bourne`). Relay-bound strings also pass straight to
/// the socket with no re-encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayCommand {
    /// Connect the pool to this set of relay URLs.
    Connect(Vec<String>),
    /// Add a single relay to the pool at runtime.
    AddRelay(String),
    /// Remove a relay (by URL) from the pool.
    RemoveRelay(String),
    /// Begin matching/streaming notes for this subscription.
    ///
    /// The main thread builds the REQ id (so the id it hands callers matches the
    /// one relays see); the worker records `filter_json` keyed by `sub_id`,
    /// forwards `req_json` to every open socket, and matches incoming notes
    /// against the filter.
    Subscribe {
        sub_id: String,
        filter_json: String,
        req_json: String,
    },
    /// Stop matching for `sub_id` and send CLOSE to relays.
    Close(String),
    /// Forward a raw client event frame to every open relay (e.g. a signed note
    /// to publish, or an AUTH response).
    Send(String),
    /// Synthetic load, for `worker_bench`. Frames run through the same
    /// ingestor the sockets use, so the benchmark measures this exact path.
    #[cfg(feature = "bench-harness")]
    Flood(super::flood::FloodSpec),
}

/// What the reactor sends OUT to the application (main thread).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerOut {
    /// A deduped note that matched at least one active filter.
    ///
    /// Carries the relay's own EVENT frame verbatim, not a re-encoded note: the
    /// worker already parsed it to dedup and match, so forwarding the original
    /// string costs no second serialization.
    Note(String),
    /// A non-note relay event frame (EOSE / OK / NOTICE / AUTH / CLOSED),
    /// forwarded verbatim so `use_relay_events` consumers still see them.
    RelayEvent(String),
    /// Per-relay connection health, pushed whenever a socket opens or closes so
    /// `relay_health()` on the main thread stays current.
    RelayHealth(Vec<(String, ReadyState)>),
}

/// Internal worker-loop events fanned in from the sync JS socket callbacks.
///
/// `Out` carries something to ship; `Reconnect` asks the loop to re-create a
/// socket (so the *loop* keeps owning every socket — health and `RemoveRelay`
/// stay correct across reconnects).
pub enum Internal {
    Out(Ingested),
    Health(Vec<(String, ReadyState)>),
    Reconnect {
        url: String,
        retry_count: u32,
    },
    /// Check the inbound ring for commands.
    ///
    /// The bridge wakes the loop by itself, but a ring is passive memory —
    /// nothing arrives to wake anyone. A ticker posts this so ring commands get
    /// picked up, and it is what makes the two transports interchangeable to
    /// the loop.
    Poll,
}

/// The reactor entry: owns the socket pool, dedups + filters notes, and streams
/// matches + relay events + health back over the bridge.
///
/// Input  = [`RelayCommand`]
/// Output = [`WorkerOut`]
#[reactor]
pub async fn RelayReactor(mut scope: ReactorScope<RelayCommand, WorkerOut>) {
    // Worker-internal event bus. The sync JS callbacks push `Internal`s here;
    // the async loop drains it. `unbounded` so a burst never drops a note.
    let (bus_tx, mut bus_rx) = mpsc::unbounded::<Internal>();
    let mut relays = RelaySet::new(NoteIngestor::new(10_000), bus_tx.clone());
    let sink = WorkerSink::new(super::worker_boot::WorkerRings::transport());

    // Only the rings need waking; the bridge wakes this loop by itself. Held
    // for the life of the loop, so the timer stops when the worker stops.
    let _poller = sink
        .uses_shared_rings()
        .then(|| super::ring_poller::RingPoller::start(bus_tx))
        .flatten();

    loop {
        futures::select! {
            // A command arrived over the bridge.
            cmd = scope.next() => {
                let Some(command) = cmd else {
                    break; // bridge closed — the application dropped the pool
                };
                relays.apply(command);
            }
            // An internal event from a socket callback.
            ev = bus_rx.next() => {
                match ev {
                    Some(Internal::Out(ingested)) => {
                        if !sink.ship(ingested, &mut scope).await {
                            break; // peer gone
                        }
                    }
                    Some(Internal::Health(health)) => {
                        if !sink.ship_health(health, &mut scope).await {
                            break;
                        }
                    }
                    Some(Internal::Reconnect { url, retry_count }) => {
                        relays.reconnect(&url, retry_count);
                    }
                    Some(Internal::Poll) => {
                        for command in sink.poll_commands() {
                            relays.apply(command);
                        }
                        sink.flush();
                    }
                    None => break,
                }
            }
        }
    }
}

/// A WebSocket living inside the worker. Owns its JS closures (so they stay
/// attached) and closes the socket on drop. Parses, dedups, and filter-matches
/// every message; only matched notes (and non-note relay events) cross the
/// bridge. On close it asks the reactor loop to reconnect with bounded backoff.
pub struct WorkerSocket {
    ws: web_sys::WebSocket,
    url: String,
    _onopen: Closure<dyn FnMut()>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _onclose: Closure<dyn FnMut(web_sys::CloseEvent)>,
    _onerror: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    queue: Rc<RefCell<Vec<String>>>,
}

impl Drop for WorkerSocket {
    fn drop(&mut self) {
        self.ws.set_onopen(None);
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        self.ws.set_onerror(None);
        let _ = self.ws.close();
    }
}

impl WorkerSocket {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn ready_state(&self) -> ReadyState {
        ReadyState::from_web_sys(self.ws.ready_state())
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn connect(
        url: String,
        retry_count: u32,
        ingestor: NoteIngestor,
        bus_tx: mpsc::UnboundedSender<Internal>,
    ) -> Result<Self, JsValue> {
        let ws = web_sys::WebSocket::new(&url)?;
        let queue: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // On open: re-send every active REQ (so a reconnected socket resumes its
        // subscriptions), then flush anything queued before the socket opened.
        // Also report health so the main thread sees OPEN.
        let onopen = {
            let sender = ws.clone();
            let queue = queue.clone();
            let ingestor = ingestor.clone();
            let bus_tx = bus_tx.clone();
            let url = url.clone();
            Closure::wrap(Box::new(move || {
                for req in ingestor.active_reqs() {
                    if let Ok(req_str) = crate::NostrJson::to_string(&req) {
                        let _ = sender.send_with_str(&req_str);
                    }
                }
                for msg in queue.borrow().iter() {
                    let _ = sender.send_with_str(msg);
                }
                queue.borrow_mut().clear();
                let _ =
                    bus_tx.unbounded_send(Internal::Health(vec![(url.clone(), ReadyState::OPEN)]));
            }) as Box<dyn FnMut()>)
        };

        // On message: parse → (for notes) dedup → filter-match. Matched notes
        // and all non-note relay events are pushed to the bridge channel.
        let onmessage = {
            let bus_tx = bus_tx.clone();
            Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                let Some(raw) = e.data().as_string() else {
                    return;
                };
                if let Some(out) = ingestor.ingest(raw) {
                    let _ = bus_tx.unbounded_send(Internal::Out(out));
                }
            }) as Box<dyn FnMut(web_sys::MessageEvent)>)
        };

        // On close: ask the reactor loop to reconnect after bounded backoff.
        let onclose = {
            let url = url.clone();
            let bus_tx = bus_tx;
            Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
                let _ = bus_tx
                    .unbounded_send(Internal::Health(vec![(url.clone(), ReadyState::CLOSED)]));

                if retry_count >= MAX_RETRIES {
                    return;
                }
                let next = retry_count + 1;
                let timeout_secs = 2_u32.saturating_pow(next).min(MAX_RECONNECT_SECS);
                let url = url.clone();
                let bus_tx = bus_tx.clone();
                yew::platform::spawn_local(async move {
                    yew::platform::time::sleep(Duration::from_secs(u64::from(timeout_secs))).await;
                    let _ = bus_tx.unbounded_send(Internal::Reconnect {
                        url,
                        retry_count: next,
                    });
                });
            }) as Box<dyn FnMut(web_sys::CloseEvent)>)
        };

        let onerror = Closure::wrap(
            Box::new(move |_e: web_sys::ErrorEvent| {}) as Box<dyn FnMut(web_sys::ErrorEvent)>
        );

        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            ws,
            url,
            _onopen: onopen,
            _onmessage: onmessage,
            _onclose: onclose,
            _onerror: onerror,
            queue,
        })
    }

    /// Send now if open, otherwise queue until `onopen` fires.
    pub fn send_str(&self, msg: &str) {
        if self.ws.ready_state() == web_sys::WebSocket::OPEN {
            let _ = self.ws.send_with_str(msg);
        } else {
            self.queue.borrow_mut().push(msg.to_string());
        }
    }
}
