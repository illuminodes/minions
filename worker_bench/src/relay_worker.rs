//! The relay reactor: a yew-agent Reactor that runs in a Web Worker.
//!
//! It holds the WebSocket pool, the bounded dedup tracker, and performs
//! client-side filter matching — all OFF the main thread. Matched notes are
//! streamed back to the application over the reactor bridge.
//!
//! Compared to `nostr-minions`' in-thread pool, the tradeoff is explicit:
//! we move JSON parsing + dedup + filter-matching off the UI thread, but pay
//! a bincode (de)serialization + postMessage cost per note crossing the
//! worker boundary. The benchmark exists to measure whether that trade is a
//! net win under load.

use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use nostro2::{NostrNote, NostrRelayEvent, NostrSubscription};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew_agent::prelude::*;

/// Messages the application sends INTO the worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayCommand {
    /// Connect the pool to this set of relay URLs. Sent once on startup.
    Connect(Vec<String>),
    /// Begin matching/streaming notes for this subscription filter.
    Subscribe(NostrSubscription),
}

/// The reactor: connects to relays inside the worker, dedups + filters notes,
/// and streams matches back to the bridge.
///
/// Input  = [`RelayCommand`]
/// Output = [`NostrNote`]
#[reactor]
pub async fn RelayReactor(mut scope: ReactorScope<RelayCommand, NostrNote>) {
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
                    // Bridge closed — application dropped the subscription.
                    None => break,
                }
            }
            // A matched note is ready to ship back to the application.
            note = note_rx.next() => {
                if let Some(note) = note {
                    // If the bridge is gone, stop.
                    if scope.send(note).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
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
