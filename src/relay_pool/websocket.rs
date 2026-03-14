use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const MAX_RECONNECT_SECS: u32 = 120;
const MAX_RETRIES: u32 = 10;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ReadyState {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}

impl ReadyState {
    const fn from_web_sys(state: u16) -> Self {
        match state {
            0 => Self::CONNECTING,
            1 => Self::OPEN,
            2 => Self::CLOSING,
            _ => Self::CLOSED,
        }
    }
}

/// Inner WebSocket state — cleaned up only when the last reference is dropped.
/// This prevents the old bug where cloning `NostrWebSocket` and dropping the
/// clone would detach handlers and close the shared underlying socket.
struct WebSocketInner {
    websocket: web_sys::WebSocket,
    _onopen: Closure<dyn FnMut()>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _onclose: Closure<dyn FnMut(web_sys::CloseEvent)>,
    _onerror: Closure<dyn FnMut(web_sys::ErrorEvent)>,
}

impl Drop for WebSocketInner {
    fn drop(&mut self) {
        self.websocket.set_onopen(None);
        self.websocket.set_onmessage(None);
        self.websocket.set_onclose(None);
        self.websocket.set_onerror(None);
        let _ = self.websocket.close();
    }
}

/// WebSocket wrapper with proper lifecycle management.
///
/// Uses `Rc<WebSocketInner>` so clones share the underlying socket and
/// cleanup only happens when the last clone is dropped.
#[derive(Clone)]
pub struct NostrWebSocket {
    inner: std::rc::Rc<WebSocketInner>,
    pub url: String,
    pub ready_state: ReadyState,
    pub queue: std::rc::Rc<std::cell::RefCell<Vec<nostro2::NostrClientEvent>>>,
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
        self.url == other.url && self.ready_state == other.ready_state
    }
}

impl Eq for NostrWebSocket {}

impl NostrWebSocket {
    /// Get a reference to the underlying `web_sys::WebSocket`.
    pub fn websocket(&self) -> &web_sys::WebSocket {
        &self.inner.websocket
    }

    /// Read the actual ready state from the underlying WebSocket,
    /// bypassing the cached `ready_state` field. Use this in `send()`
    /// to avoid the race between `onopen` firing and the reducer
    /// dispatch updating the cached field.
    pub fn actual_ready_state(&self) -> ReadyState {
        ReadyState::from_web_sys(self.inner.websocket.ready_state())
    }

    /// Connect to a Nostr relay with automatic retry on failure.
    ///
    /// Creates a fresh retry counter. Reconnections on close will use
    /// exponential backoff and give up after `MAX_RETRIES` attempts.
    /// The counter resets on every successful open.
    ///
    /// # Errors
    /// Returns error if WebSocket creation fails.
    #[allow(clippy::needless_pass_by_value)]
    pub fn connect(
        url: String,
        dispatch: yew::UseReducerDispatcher<super::NostrRelayPool>,
        note_dedup: std::rc::Rc<std::cell::RefCell<super::BoundedDedup>>,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let retry_count = std::rc::Rc::new(std::cell::Cell::new(0_u32));
        Self::connect_inner(url, dispatch, note_dedup, retry_count)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn connect_inner(
        url: String,
        dispatch: yew::UseReducerDispatcher<super::NostrRelayPool>,
        note_dedup: std::rc::Rc<std::cell::RefCell<super::BoundedDedup>>,
        retry_count: std::rc::Rc<std::cell::Cell<u32>>,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let ws = web_sys::WebSocket::new(&url)?;
        let queue = std::rc::Rc::new(std::cell::RefCell::new(vec![]));

        // Create onopen handler
        let onopen = {
            let dispatch = dispatch.clone();
            let url = url.clone();
            let sender = ws.clone();
            let queue = queue.clone();
            let retry_count = retry_count.clone();

            Closure::wrap(Box::new(move || {
                // Reset retry counter on successful connection
                retry_count.set(0);

                dispatch.dispatch(super::NostrRelayPoolAction::Open(url.clone()));

                // Send all queued messages
                for event in queue.borrow().iter() {
                    if let Ok(event_str) = serde_json::to_string(event) {
                        let _ = sender.send_with_str(&event_str);
                    }
                }
                queue.borrow_mut().clear();
            }) as Box<dyn FnMut()>)
        };

        // Create onmessage handler — dispatches ALL relay events, not just notes
        let onmessage = {
            let dispatch = dispatch.clone();
            let note_dedup = note_dedup.clone();

            Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                let Some(data) = e
                    .data()
                    .as_string()
                    .and_then(|s| s.parse::<nostro2::NostrRelayEvent>().ok())
                else {
                    return;
                };

                // Dedup notes specifically
                if let nostro2::NostrRelayEvent::NewNote(.., ref note) = data {
                    if let Some(ref note_id) = note.id {
                        if !note_dedup.borrow_mut().insert(note_id.clone()) {
                            return; // Skip duplicate
                        }
                    }
                }

                dispatch.dispatch(super::NostrRelayPoolAction::RelayEvent(data));
            }) as Box<dyn FnMut(web_sys::MessageEvent)>)
        };

        // Create onclose handler with bounded reconnection
        #[allow(clippy::redundant_clone)]
        let onclose = {
            let url = url.clone();
            let dispatch = dispatch.clone();
            let note_dedup = note_dedup.clone();
            let retry_count = retry_count.clone();

            Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
                dispatch.dispatch(super::NostrRelayPoolAction::CloseRelay(url.clone()));

                let retries = retry_count.get();
                if retries >= MAX_RETRIES {
                    return; // Give up after MAX_RETRIES attempts
                }
                retry_count.set(retries + 1);

                let timeout_secs = 2_u32.saturating_pow(retries + 1).min(MAX_RECONNECT_SECS);
                let url = url.clone();
                let dispatch = dispatch.clone();
                let note_dedup = note_dedup.clone();
                let retry_count = retry_count.clone();

                yew::platform::spawn_local(async move {
                    yew::platform::time::sleep(std::time::Duration::from_secs(timeout_secs.into()))
                        .await;

                    if let Ok(ws) =
                        Self::connect_inner(url, dispatch.clone(), note_dedup, retry_count)
                    {
                        dispatch.dispatch(super::NostrRelayPoolAction::Reconnected(ws));
                    }
                });
            }) as Box<dyn FnMut(web_sys::CloseEvent)>)
        };

        // Create onerror handler
        let onerror = {
            Closure::wrap(
                Box::new(move |_e: web_sys::ErrorEvent| {}) as Box<dyn FnMut(web_sys::ErrorEvent)>
            )
        };

        // Attach handlers to websocket
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            inner: std::rc::Rc::new(WebSocketInner {
                websocket: ws,
                _onopen: onopen,
                _onmessage: onmessage,
                _onclose: onclose,
                _onerror: onerror,
            }),
            url,
            ready_state: ReadyState::CONNECTING,
            queue,
        })
    }
}
