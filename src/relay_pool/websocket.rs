#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ReadyState {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NostrWebSocket {
    pub websocket: web_sys::WebSocket,
    pub url: String,
    pub ready_state: ReadyState,
    pub queue: std::rc::Rc<std::cell::RefCell<Vec<nostro2::NostrClientEvent>>>,
}
impl NostrWebSocket {
    pub fn set_onopen(&self, onopen: impl FnOnce() + 'static) {
        self.websocket
            .set_onopen(Some(wasm_bindgen::JsCast::unchecked_ref(
                &wasm_bindgen::closure::Closure::once_into_js(onopen),
            )));
    }
    pub fn set_onclose(&self, onclose: impl FnOnce(web_sys::CloseEvent) + 'static) {
        self.websocket
            .set_onclose(Some(wasm_bindgen::JsCast::unchecked_ref(
                &wasm_bindgen::closure::Closure::once_into_js(onclose),
            )));
    }
    pub fn set_onerror(&self, onerror: impl FnMut(web_sys::ErrorEvent) + 'static) {
        self.websocket
            .set_onerror(Some(wasm_bindgen::JsCast::unchecked_ref(
                &wasm_bindgen::closure::Closure::wrap(Box::new(onerror) as Box<dyn FnMut(_)>)
                    .into_js_value(),
            )));
    }
    pub fn set_onmessage(&self, onmessage: impl FnMut(web_sys::MessageEvent) + 'static) {
        self.websocket
            .set_onmessage(Some(wasm_bindgen::JsCast::unchecked_ref(
                &wasm_bindgen::closure::Closure::wrap(Box::new(onmessage) as Box<dyn FnMut(_)>)
                    .into_js_value(),
            )));
    }
    pub fn connect_with_retry(
        url: String,
        dispatch: yew::UseReducerDispatcher<super::NostrRelayPool>,
        note_lib: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>>,
        timeout: u32,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let ws = web_sys::WebSocket::new(&url)?;
        let nostr_relay = Self {
            websocket: ws,
            url,
            ready_state: ReadyState::CONNECTING,
            queue: std::rc::Rc::new(std::cell::RefCell::new(vec![])),
        };

        {
            let dispatch = dispatch.clone();
            let relay = nostr_relay.clone();
            let sender = relay.websocket.clone();
            nostr_relay.set_onopen(move || {
                dispatch.dispatch(super::NostrRelayPoolAction::Open(relay.url.clone()));
                for event in relay.queue.borrow().iter() {
                    if let Ok(event_str) = serde_json::to_string(event) {
                        if let Err(e) = sender.send_with_str(&event_str) {
                            web_sys::console::error_1(&e);
                        }
                    }
                }
                relay.queue.borrow_mut().clear();
            });
        }

        // On message
        {
            let dispatch = dispatch.clone();
            let note_lib = note_lib.clone();
            nostr_relay.set_onmessage(move |e: web_sys::MessageEvent| {
                let Some(data) = e
                    .data()
                    .as_string()
                    .and_then(|s| s.parse::<nostro2::NostrRelayEvent>().ok())
                else {
                    web_sys::console::error_1(&format!("Invalid message: {e:?}").into());
                    return;
                };
                if let nostro2::NostrRelayEvent::NewNote(_tag, _id, note) = data {
                    if let Some(ref note_id) = note.id {
                        if note_lib.borrow().contains(note_id.as_str()) {
                            return;
                        }
                        note_lib.borrow_mut().insert(note_id.clone());
                    }
                    dispatch.dispatch(super::NostrRelayPoolAction::NewNote(note));
                } else {
                    dispatch.dispatch(super::NostrRelayPoolAction::NewEvent(data));
                }
            });
        }

        // On close/error → exponential backoff reconnect
        let schedule_reconnect = {
            let relay_url = nostr_relay.url.clone();
            move || {
                let next_wait = timeout * 2;
                let dispatch_clone = dispatch.clone();
                let url_clone = relay_url.clone();
                let note_lib_clone = note_lib.clone();
                yew::platform::spawn_local(async move {
                    yew::platform::time::sleep(std::time::Duration::from_secs(next_wait.into()))
                        .await;

                    let _ = Self::connect_with_retry(
                        url_clone.clone(),
                        dispatch_clone.clone(),
                        note_lib_clone,
                        next_wait,
                    );
                });
            }
        };

        nostr_relay.set_onclose(move |e: web_sys::CloseEvent| {
            if !e.was_clean() {
                schedule_reconnect();
            }
        });

        nostr_relay.set_onerror(move |_e: web_sys::ErrorEvent| {
            // web_sys::console::error_1(&e);
        });
        Ok(nostr_relay)
    }
}
