//! The relay reactor: a yew-agent Reactor that runs in a Web Worker.
//!
//! It holds the WebSocket pool, the bounded dedup tracker, and performs
//! client-side filter matching — all OFF the main thread. Matched notes are
//! streamed back to the application over the reactor bridge.
//!
//! Compared to `nostr-minions`' in-thread pool, the tradeoff is explicit:
//! we move JSON parsing + dedup + filter-matching off the UI thread, but pay
//! a (de)serialization + postMessage cost per note crossing the worker
//! boundary. The benchmark exists to measure whether that trade is a net win
//! under load.

#[path = "metrics.rs"]
pub mod metrics;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use nostro2::{NostrNote, NostrRelayEvent, NostrSubscription};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew_agent::prelude::*;

/// JSON codec for the worker bridge.
///
/// yew-agent's default `Bincode` codec panics on the `nostro2` types
/// (`SequenceMustHaveLength`) — their serde representation uses patterns
/// bincode can't encode without an up-front length (e.g. flattened/sequence
/// fields). JSON has no such requirement, so both the provider and the
/// registrar use this codec. (JSON is also closer to what the in-thread path
/// already parses, keeping the comparison honest.)
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
        let s = input.as_string().expect("worker message: expected string");
        serde_json::from_str(&s).expect("worker message: deserialize")
    }
}

/// Messages the application sends INTO the worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RelayCommand {
    /// Connect the pool to this set of relay URLs. Sent once on startup.
    Connect(Vec<String>),
    /// Begin matching/streaming notes for this subscription filter.
    Subscribe(NostrSubscription),
    /// Synthetic load: generate `rate` notes/sec for `secs` seconds, parsing
    /// each through the SAME path as real relay messages (dedup + filter), so
    /// the cost being measured is the worker's real ingestion work — not a
    /// shortcut. `start_seq` keeps note ids unique across runs.
    Flood {
        rate: u32,
        secs: u32,
        start_seq: u64,
        /// Extra bytes padded into each note's content (message-size axis).
        payload_bytes: usize,
    },
}

/// What the reactor sends OUT to the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerOut {
    /// A matched note.
    Note(NostrNote),
    /// Worker-side queue depth: notes that have been matched + enqueued but not
    /// yet shipped across the bridge. This is the "hidden" backlog that the
    /// app's own backlog metric can't see — when it climbs under load, the
    /// bridge (serialize + postMessage) is the bottleneck, which is what drives
    /// rising end-to-end latency at large payloads.
    QueueDepth(u64),
}

/// The reactor: connects to relays inside the worker, dedups + filters notes,
/// and streams matches back to the bridge.
///
/// Input  = [`RelayCommand`]
/// Output = [`WorkerOut`]
#[reactor]
pub async fn RelayReactor(mut scope: ReactorScope<RelayCommand, WorkerOut>) {
    // Bounded dedup, shared with each socket's onmessage closure.
    let dedup = Rc::new(RefCell::new(nostr_minions::BoundedDedup::new(10_000)));

    // Active filters we are matching against, shared with the closures.
    let filters: Rc<RefCell<Vec<NostrSubscription>>> = Rc::new(RefCell::new(Vec::new()));

    // Sockets kept alive for the lifetime of the reactor. The closures inside
    // each socket are owned here so they aren't dropped (which would detach
    // the handlers).
    let mut sockets: Vec<WorkerSocket> = Vec::new();

    // Notes flow from the socket onmessage closures into this channel, then
    // back out to the bridge below. An mpsc channel lets the sync JS callback
    // hand work to the async reactor loop.
    let (note_tx, mut note_rx) = mpsc::unbounded::<NostrNote>();

    // Queue-depth accounting: `enqueued` is bumped wherever a note is pushed
    // into note_tx (flood + sockets); `shipped` is bumped after each successful
    // scope.send. depth = enqueued - shipped = notes waiting in the bridge.
    let enqueued = Rc::new(std::cell::Cell::new(0_u64));
    let mut shipped = 0_u64;

    loop {
        futures::select! {
            // A command arrived from the application.
            cmd = scope.next() => {
                match cmd {
                    Some(RelayCommand::Connect(urls)) => {
                        for url in urls {
                            if let Ok(sock) = WorkerSocket::connect(
                                &url,
                                dedup.clone(),
                                filters.clone(),
                                note_tx.clone(),
                            ) {
                                sockets.push(sock);
                            }
                        }
                    }
                    Some(RelayCommand::Subscribe(filter)) => {
                        // Send the REQ to every open socket and record the
                        // filter so onmessage can match against it.
                        let req: nostro2::NostrClientEvent = filter.clone().into();
                        if let Ok(req_str) = serde_json::to_string(&req) {
                            for sock in &sockets {
                                sock.send_str(&req_str);
                            }
                        }
                        filters.borrow_mut().push(filter);
                    }
                    Some(RelayCommand::Flood { rate, secs, start_seq, payload_bytes }) => {
                        // Run the flood as a detached task feeding the SAME
                        // channel, so the reactor loop stays responsive and
                        // notes arrive spread over time (not one giant burst).
                        spawn_flood(
                            rate,
                            secs,
                            start_seq,
                            payload_bytes,
                            dedup.clone(),
                            filters.clone(),
                            note_tx.clone(),
                            enqueued.clone(),
                        );
                    }
                    // Bridge closed — application dropped the subscription.
                    None => break,
                }
            }
            // A matched note is ready to ship back to the application.
            note = note_rx.next() => {
                if let Some(note) = note {
                    // If the bridge is gone, stop.
                    if scope.send(WorkerOut::Note(note)).await.is_err() {
                        break;
                    }
                    shipped += 1;
                    // Report the bridge queue depth every 64 notes (cheap, and
                    // frequent enough to watch it climb without spamming the
                    // bridge with depth reports that would themselves queue).
                    if shipped % 64 == 0 {
                        let depth = enqueued.get().saturating_sub(shipped);
                        if scope.send(WorkerOut::QueueDepth(depth)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Detached synthetic-load task. Generates `rate` notes/sec for `secs` seconds,
/// parsing each through the SAME `NostrRelayEvent` parse + dedup + filter path
/// as a real relay message, then feeding matches into `note_tx`.
///
/// Notes are emitted in ~16ms ticks (one animation frame) so they spread over
/// wall-clock time rather than arriving as a single blocking burst — matching
/// how a real high-rate feed behaves.
#[allow(clippy::too_many_arguments)] // benchmark plumbing; not a public API
fn spawn_flood(
    rate: u32,
    secs: u32,
    start_seq: u64,
    payload_bytes: usize,
    dedup: Rc<RefCell<nostr_minions::BoundedDedup>>,
    filters: Rc<RefCell<Vec<NostrSubscription>>>,
    note_tx: mpsc::UnboundedSender<NostrNote>,
    enqueued: Rc<std::cell::Cell<u64>>,
) {
    yew::platform::spawn_local(async move {
        const TICK_MS: u32 = 16;
        let ticks = secs * (1000 / TICK_MS);
        let per_tick = (rate * TICK_MS / 1000).max(1);
        let mut seq = start_seq;

        for _ in 0..ticks {
            // Wall-clock epoch: the app reads it back on a DIFFERENT thread, so
            // the timestamp must be cross-context comparable (see metrics::wall_ms).
            let emit = metrics::wall_ms();
            for _ in 0..per_tick {
                let raw = metrics::synthetic_event(seq, emit, payload_bytes);
                seq += 1;
                // Same ingestion path as onmessage: parse → dedup → filter.
                let Ok(NostrRelayEvent::NewNote(.., note)) = raw.parse::<NostrRelayEvent>() else {
                    continue;
                };
                if let Some(ref id) = note.id {
                    if !dedup.borrow_mut().insert(id.clone()) {
                        continue;
                    }
                }
                let matched = filters
                    .borrow()
                    .iter()
                    .any(|f| nostr_minions::note_matches_filter(&note, f));
                if matched {
                    if note_tx.unbounded_send(note).is_err() {
                        return; // bridge gone
                    }
                    enqueued.set(enqueued.get() + 1);
                }
            }
            yew::platform::time::sleep(Duration::from_millis(u64::from(TICK_MS))).await;
        }
    });
}

/// A raw WebSocket living inside the worker. Keeps its closures alive and
/// closes the socket on drop.
struct WorkerSocket {
    ws: web_sys::WebSocket,
    _onopen: Closure<dyn FnMut()>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
    queue: Rc<RefCell<Vec<String>>>,
}

impl Drop for WorkerSocket {
    fn drop(&mut self) {
        self.ws.set_onopen(None);
        self.ws.set_onmessage(None);
        let _ = self.ws.close();
    }
}

impl WorkerSocket {
    fn connect(
        url: &str,
        dedup: Rc<RefCell<nostr_minions::BoundedDedup>>,
        filters: Rc<RefCell<Vec<NostrSubscription>>>,
        note_tx: mpsc::UnboundedSender<NostrNote>,
    ) -> Result<Self, JsValue> {
        let ws = web_sys::WebSocket::new(url)?;
        let queue: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // Flush any REQs queued before the socket opened.
        let onopen = {
            let sender = ws.clone();
            let queue = queue.clone();
            Closure::wrap(Box::new(move || {
                for msg in queue.borrow().iter() {
                    let _ = sender.send_with_str(msg);
                }
                queue.borrow_mut().clear();
            }) as Box<dyn FnMut()>)
        };

        // Parse, dedup, filter — all on the worker thread. Only matched notes
        // cross the channel toward the bridge.
        let onmessage = {
            Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                let Some(data) = e
                    .data()
                    .as_string()
                    .and_then(|s| s.parse::<NostrRelayEvent>().ok())
                else {
                    return;
                };
                let NostrRelayEvent::NewNote(.., note) = data else {
                    return; // EOSE/OK/NOTICE/etc. are irrelevant to the bench
                };
                if let Some(ref id) = note.id {
                    if !dedup.borrow_mut().insert(id.clone()) {
                        return; // duplicate across relays
                    }
                }
                // Match against every active filter; emit once if any matches.
                let matched = filters
                    .borrow()
                    .iter()
                    .any(|f| nostr_minions::note_matches_filter(&note, f));
                if matched {
                    let _ = note_tx.unbounded_send(note);
                }
            }) as Box<dyn FnMut(web_sys::MessageEvent)>)
        };

        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        Ok(Self {
            ws,
            _onopen: onopen,
            _onmessage: onmessage,
            queue,
        })
    }

    /// Send now if open, otherwise queue until onopen fires.
    fn send_str(&self, msg: &str) {
        if self.ws.ready_state() == web_sys::WebSocket::OPEN {
            let _ = self.ws.send_with_str(msg);
        } else {
            self.queue.borrow_mut().push(msg.to_string());
        }
    }
}
