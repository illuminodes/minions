use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ReadyState {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}
#[derive(Debug, PartialEq, Clone)]
struct NostrRelay {
    websocket: web_sys::WebSocket,
    url: String,
    ready_state: ReadyState,
    queue: std::rc::Rc<std::cell::RefCell<Vec<nostro2::NostrClientEvent>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NostrRelayPool {
    pool: std::rc::Rc<std::cell::RefCell<Vec<NostrRelay>>>,
    pub unique_notes: Vec<nostro2::NostrNote>,
    pub relay_events: Vec<nostro2::NostrRelayEvent>,
}
impl NostrRelayPool {
    #[must_use]
    pub fn relay_health(&self) -> std::collections::HashMap<String, ReadyState> {
        let mut health = std::collections::HashMap::new();
        for relay in self.pool.borrow().iter() {
            health.insert(relay.url.clone(), relay.ready_state);
        }
        health
    }
    pub fn send<T>(&self, event: T) -> nostro2::NostrClientEvent
    where
        T: Into<nostro2::NostrClientEvent> + Clone,
    {
        let event = event.into();
        for relay in self.pool.borrow().iter() {
            match relay.ready_state {
                ReadyState::CONNECTING => {
                    relay.queue.borrow_mut().push(event.clone());
                }
                ReadyState::OPEN => {
                    if let Ok(event_str) = serde_json::to_string(&event) {
                        if let Err(e) = relay.websocket.send_with_str(&event_str) {
                            web_sys::console::error_1(&e);
                        }
                    }
                }
                _ => {}
            }
        }
        event
    }
}

pub enum NostrRelayPoolAction {
    Open(String),
    NewEvent(nostro2::NostrRelayEvent),
    NewNote(nostro2::NostrNote),
    CloseRelay(String),
    AddRelay(crate::relay_pool::UserRelay),
    RemoveRelay(crate::relay_pool::UserRelay),
}
impl Reducible for NostrRelayPool {
    type Action = NostrRelayPoolAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            NostrRelayPoolAction::AddRelay(relay) => {
                let mut pool = self.pool.borrow_mut();
                if let Ok(ws) = web_sys::WebSocket::new(&relay.url) {
                    let ws = NostrRelay {
                        websocket: ws,
                        url: relay.url.clone(),
                        ready_state: ReadyState::CONNECTING,
                        queue: std::rc::Rc::new(std::cell::RefCell::new(vec![])),
                    };
                    pool.push(ws);
                }
                std::rc::Rc::new(Self {
                    pool: std::rc::Rc::new(std::cell::RefCell::new(pool.to_vec())),
                    unique_notes: self.unique_notes.clone(),
                    relay_events: self.relay_events.clone(),
                })
            }
            NostrRelayPoolAction::RemoveRelay(relay) => {
                let mut pool = self.pool.borrow_mut();
                pool.retain(|r| r.url != relay.url);
                std::rc::Rc::new(Self {
                    pool: std::rc::Rc::new(std::cell::RefCell::new(pool.to_vec())),
                    unique_notes: self.unique_notes.clone(),
                    relay_events: self.relay_events.clone(),
                })
            }
            NostrRelayPoolAction::Open(url) => {
                let mut pool = self.pool.borrow_mut();
                for relay in pool.iter_mut() {
                    if relay.url == url {
                        relay.ready_state = ReadyState::OPEN;
                    }
                }
                self.clone()
            }
            NostrRelayPoolAction::NewEvent(event) => {
                let mut relay_events = self.relay_events.clone();
                relay_events.push(event);
                Self {
                    pool: self.pool.clone(),
                    unique_notes: self.unique_notes.clone(),
                    relay_events,
                }
                .into()
            }
            NostrRelayPoolAction::NewNote(note) => {
                let mut unique_notes = self.unique_notes.clone();
                unique_notes.push(note);
                Self {
                    pool: self.pool.clone(),
                    unique_notes,
                    relay_events: self.relay_events.clone(),
                }
                .into()
            }
            NostrRelayPoolAction::CloseRelay(url) => {
                let mut pool = self.pool.borrow_mut();
                for relay in pool.iter_mut() {
                    if relay.url == url {
                        relay.ready_state = ReadyState::CLOSED;
                    }
                }
                self.clone()
            }
        }
    }
}
pub type NostrRelayPoolStore = UseReducerHandle<NostrRelayPool>;

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct RelayContextProps {
    pub children: Children,
    pub relays: Vec<crate::relay_pool::UserRelay>,
}

#[function_component(NostrRelayPoolProvider)]
pub fn key_handler(props: &RelayContextProps) -> Html {
    let pool = use_mut_ref(|| {
        let mut pool = vec![];
        for relay in &props.relays {
            if let Ok(ws) = web_sys::WebSocket::new(&relay.url) {
                let ws = NostrRelay {
                    websocket: ws,
                    url: relay.url.clone(),
                    ready_state: ReadyState::CONNECTING,
                    queue: std::rc::Rc::new(std::cell::RefCell::new(vec![])),
                };
                pool.push(ws);
            }
        }
        pool
    });
    let ctx = use_reducer(|| NostrRelayPool {
        pool: pool.clone(),
        unique_notes: vec![],
        relay_events: vec![],
    });
    let ctx_clone = ctx.clone();
    let note_lib: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>> =
        use_mut_ref(std::collections::HashSet::new);
    use_memo(ctx_clone.pool.clone(), move |pool| {
        for relay in (pool.borrow()).iter().cloned() {
            let dispatcher = ctx_clone.dispatcher();
            let sender = relay.websocket.clone();
            let url = relay.url.clone();
            relay.websocket.set_onopen(Some(
                wasm_bindgen::closure::Closure::once_into_js(
                    move |_open: wasm_bindgen::JsValue| {
                        dispatcher.dispatch(NostrRelayPoolAction::Open(url.clone()));
                        let queue = relay.queue.clone();
                        for event in queue.borrow().iter() {
                            if let Ok(event_str) = serde_json::to_string(event) {
                                if let Err(e) = sender.send_with_str(&event_str) {
                                    web_sys::console::error_1(&e);
                                }
                            }
                        }
                        queue.borrow_mut().clear();
                    },
                )
                .unchecked_ref(),
            ));
            let dispatcher = ctx_clone.dispatcher();
            let note_lib = note_lib.clone();
            relay.websocket.set_onmessage(Some(
                wasm_bindgen::closure::Closure::wrap(Box::new(
                    move |event: web_sys::MessageEvent| {
                        let Ok(Ok(data)) =
                            event.data().dyn_into::<wasm_bindgen::JsValue>().map(|v| {
                                v.as_string()
                                    .unwrap_or_default()
                                    .parse::<nostro2::NostrRelayEvent>()
                            })
                        else {
                            web_sys::console::error_1(&event);
                            return;
                        };
                        if let nostro2::NostrRelayEvent::NewNote(_tag, _id, note) = data {
                            if let Some(ref note_id) = note.id {
                                if note_lib.borrow().contains(note_id.as_str()) {
                                    return;
                                }
                                note_lib.borrow_mut().insert(note_id.clone());
                            }
                            dispatcher.dispatch(NostrRelayPoolAction::NewNote(note));
                        } else {
                            dispatcher.dispatch(NostrRelayPoolAction::NewEvent(data));
                        }
                    },
                ) as Box<dyn FnMut(_)>)
                .into_js_value()
                .unchecked_ref(),
            ));
            let dispatcher = ctx_clone.dispatcher();
            relay.websocket.set_onclose(Some(
                wasm_bindgen::closure::Closure::once_into_js(move |close: web_sys::CloseEvent| {
                    web_sys::console::log_1(&close);
                    dispatcher.dispatch(NostrRelayPoolAction::CloseRelay(relay.url.clone()));
                })
                .unchecked_ref(),
            ));
            relay.websocket.set_onerror(Some(
                wasm_bindgen::closure::Closure::once_into_js(move |event: web_sys::ErrorEvent| {
                    web_sys::console::error_1(&event);
                })
                .unchecked_ref(),
            ));
        }
    });

    html! {
        <ContextProvider<NostrRelayPoolStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<NostrRelayPoolStore>>
    }
}
