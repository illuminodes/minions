use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ReadyState {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}

/// WebSocket wrapper that properly manages closure lifecycle to prevent memory leaks
#[derive(Clone)]
pub struct NostrWebSocket {
    pub websocket: web_sys::WebSocket,
    pub url: String,
    pub ready_state: ReadyState,
    pub queue: std::rc::Rc<std::cell::RefCell<Vec<nostro2::NostrClientEvent>>>,

    // Store closures to prevent memory leaks
    // Wrapped in Rc so they can be cloned with the struct
    // Will be automatically dropped when all clones are dropped
    _onopen: std::rc::Rc<Closure<dyn FnMut()>>,
    _onmessage: std::rc::Rc<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    _onclose: std::rc::Rc<Closure<dyn FnMut(web_sys::CloseEvent)>>,
    _onerror: std::rc::Rc<Closure<dyn FnMut(web_sys::ErrorEvent)>>,
}

impl std::fmt::Debug for NostrWebSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrWebSocket")
            .field("url", &self.url)
            .field("ready_state", &self.ready_state)
            .field("queued_messages", &self.queue.borrow().len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for NostrWebSocket {
    fn eq(&self, other: &Self) -> bool {
        // Compare by URL as unique identifier
        self.url == other.url
    }
}

impl Eq for NostrWebSocket {}

impl Drop for NostrWebSocket {
    fn drop(&mut self) {
        // Close websocket when dropped
        // Closures are automatically cleaned up via Rc<Closure>
        let _ = self.websocket.close();
    }
}

impl NostrWebSocket {
    /// Connect to a Nostr relay with automatic retry on failure
    ///
    /// # Arguments
    /// * `url` - WebSocket URL of the relay
    /// * `dispatch` - Yew reducer dispatcher for state updates
    /// * `note_dedup` - Bounded deduplication tracker for notes
    /// * `timeout` - Initial timeout in seconds (doubles on each retry)
    ///
    /// # Errors
    /// Returns error if WebSocket creation fails
    pub fn connect_with_retry(
        url: String,
        dispatch: yew::UseReducerDispatcher<super::NostrRelayPool>,
        note_dedup: std::rc::Rc<std::cell::RefCell<super::BoundedDedup>>,
        timeout: u32,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let ws = web_sys::WebSocket::new(&url)?;
        let queue = std::rc::Rc::new(std::cell::RefCell::new(vec![]));

        // Create onopen handler
        let onopen = {
            let dispatch = dispatch.clone();
            let url = url.clone();
            let sender = ws.clone();
            let queue = queue.clone();

            Closure::wrap(Box::new(move || {
                web_sys::console::log_1(&format!("Connected to relay: {url}").into());
                dispatch.dispatch(super::NostrRelayPoolAction::Open(url.clone()));

                // Send all queued messages
                for event in queue.borrow().iter() {
                    if let Ok(event_str) = serde_json::to_string(event) {
                        if let Err(e) = sender.send_with_str(&event_str) {
                            web_sys::console::error_1(&e);
                        }
                    }
                }
                queue.borrow_mut().clear();
            }) as Box<dyn FnMut()>)
        };

        // Create onmessage handler
        let onmessage = {
            let dispatch = dispatch.clone();
            let note_dedup = note_dedup.clone();

            Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                let Some(data) = e
                    .data()
                    .as_string()
                    .and_then(|s| s.parse::<nostro2::NostrRelayEvent>().ok())
                else {
                    web_sys::console::error_1(&format!("Invalid message: {e:?}").into());
                    return;
                };

                // Handle notes with deduplication
                if let nostro2::NostrRelayEvent::NewNote(.., ref note) = data {
                    if let Some(ref note_id) = note.id {
                        // Check and insert atomically - returns false if duplicate
                        if !note_dedup.borrow_mut().insert(note_id.clone()) {
                            return; // Skip duplicate
                        }
                    }
                    dispatch.dispatch(super::NostrRelayPoolAction::NewNote(data));
                } else {
                    dispatch.dispatch(super::NostrRelayPoolAction::NewEvent(data));
                }
            }) as Box<dyn FnMut(web_sys::MessageEvent)>)
        };

        // Create onclose handler with reconnection logic
        let onclose = {
            let url = url.clone();
            let dispatch = dispatch.clone();
            let note_dedup = note_dedup.clone();

            Closure::wrap(Box::new(move |e: web_sys::CloseEvent| {
                web_sys::console::log_1(
                    &format!("Relay closed: {} (clean: {})", url, e.was_clean()).into(),
                );
                dispatch.dispatch(super::NostrRelayPoolAction::CloseRelay(url.clone()));

                // Reconnect if not a clean close
                if !e.was_clean() {
                    let next_timeout = timeout.saturating_mul(2);
                    let url = url.clone();
                    let dispatch = dispatch.clone();
                    let note_dedup = note_dedup.clone();

                    web_sys::console::log_1(
                        &format!("Scheduling reconnect to {url} in {next_timeout}s").into(),
                    );

                    yew::platform::spawn_local(async move {
                        yew::platform::time::sleep(std::time::Duration::from_secs(
                            next_timeout.into(),
                        ))
                        .await;

                        web_sys::console::log_1(&format!("Reconnecting to {url}...").into());
                        let _ = Self::connect_with_retry(url, dispatch, note_dedup, next_timeout);
                    });
                }
            }) as Box<dyn FnMut(web_sys::CloseEvent)>)
        };

        // Create onerror handler
        let onerror = {
            let url = url.clone();
            Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
                web_sys::console::error_1(&format!("Relay error on {url}: {e:?}").into());
            }) as Box<dyn FnMut(web_sys::ErrorEvent)>)
        };

        // Attach handlers to websocket
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            websocket: ws,
            url,
            ready_state: ReadyState::CONNECTING,
            queue,
            // Store closures in Rc to prevent leaks while allowing Clone
            _onopen: std::rc::Rc::new(onopen),
            _onmessage: std::rc::Rc::new(onmessage),
            _onclose: std::rc::Rc::new(onclose),
            _onerror: std::rc::Rc::new(onerror),
        })
    }
}
