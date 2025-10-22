use yew::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NostrRelayPool {
    pool: std::rc::Rc<std::cell::RefCell<Vec<super::NostrWebSocket>>>,
    pub last_note: Option<nostro2::NostrNote>,
    pub last_event: Option<nostro2::NostrRelayEvent>,
}
impl NostrRelayPool {
    #[must_use]
    pub fn relay_health(&self) -> std::collections::HashMap<String, super::ReadyState> {
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
                super::ReadyState::CONNECTING => {
                    relay.queue.borrow_mut().push(event.clone());
                }
                super::ReadyState::OPEN => {
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
                    let ws = super::NostrWebSocket {
                        websocket: ws,
                        url: relay.url.clone(),
                        ready_state: super::ReadyState::CONNECTING,
                        queue: std::rc::Rc::new(std::cell::RefCell::new(vec![])),
                    };
                    pool.push(ws);
                }
                std::rc::Rc::new(Self {
                    pool: std::rc::Rc::new(std::cell::RefCell::new(pool.to_vec())),
                    last_note: self.last_note.clone(),
                    last_event: self.last_event.clone(),
                })
            }
            NostrRelayPoolAction::RemoveRelay(relay) => {
                let mut pool = self.pool.borrow_mut();
                pool.retain(|r| r.url != relay.url);
                std::rc::Rc::new(Self {
                    pool: std::rc::Rc::new(std::cell::RefCell::new(pool.to_vec())),
                    last_note: self.last_note.clone(),
                    last_event: self.last_event.clone(),
                })
            }
            NostrRelayPoolAction::Open(url) => {
                let mut pool = self.pool.borrow_mut();
                for relay in pool.iter_mut() {
                    if relay.url == url {
                        relay.ready_state = super::ReadyState::OPEN;
                    }
                }
                self.clone()
            }
            NostrRelayPoolAction::NewEvent(event) => Self {
                pool: self.pool.clone(),
                last_note: self.last_note.clone(),
                last_event: Some(event),
            }
            .into(),
            NostrRelayPoolAction::NewNote(note) => Self {
                pool: self.pool.clone(),
                last_note: Some(note),
                last_event: self.last_event.clone(),
            }
            .into(),
            NostrRelayPoolAction::CloseRelay(url) => {
                let mut pool = self.pool.borrow_mut();
                for relay in pool.iter_mut() {
                    if relay.url == url {
                        relay.ready_state = super::ReadyState::CLOSED;
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
    let pool = use_mut_ref(Vec::new);
    let ctx = use_reducer(|| NostrRelayPool {
        pool: pool.clone(),
        last_note: None,
        last_event: None,
    });
    let note_lib: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>> =
        use_mut_ref(std::collections::HashSet::new);
    let dispatch = ctx.dispatcher();
    let pool = ctx.pool.clone();
    use_effect_with(props.relays.clone(), move |relays| {
        for relay in relays {
            if pool.borrow().iter().any(|r| r.url == relay.url) {
                continue;
            }
            let Ok(relay_ws) = super::NostrWebSocket::connect_with_retry(
                relay.url.clone(),
                dispatch.clone(),
                note_lib.clone(),
                2,
            ) else {
                continue;
            };
            pool.borrow_mut().push(relay_ws);
        }
    });

    html! {
        <ContextProvider<NostrRelayPoolStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<NostrRelayPoolStore>>
    }
}
