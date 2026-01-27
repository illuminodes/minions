use yew::prelude::*;

#[derive(Clone)]
pub struct NostrRelayPool {
    pool: std::rc::Rc<std::cell::RefCell<Vec<super::NostrWebSocket>>>,
    pub last_note: Option<nostro2::NostrRelayEvent>,
    pub last_event: Option<nostro2::NostrRelayEvent>,
    // Store dedup tracker and pending relays for dynamic relay addition
    note_dedup: std::rc::Rc<std::cell::RefCell<super::BoundedDedup>>,
    pending_relays: std::rc::Rc<std::cell::RefCell<Vec<crate::relay_pool::UserRelay>>>,
    // Subscription management
    subscriptions: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<super::SubscriptionId, super::SubscriptionInfo>,
        >,
    >,
}

// Manual Debug impl since internal details don't need to be printed
impl std::fmt::Debug for NostrRelayPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrRelayPool")
            .field("pool_size", &self.pool.borrow().len())
            .field("last_note", &self.last_note)
            .field("last_event", &self.last_event)
            .field("note_dedup_size", &self.note_dedup.borrow().len())
            .field("pending_relays", &self.pending_relays.borrow().len())
            .field("subscriptions", &self.subscriptions.borrow().len())
            .finish()
    }
}

// Manual PartialEq impl - compare state but not internal trackers
impl PartialEq for NostrRelayPool {
    fn eq(&self, other: &Self) -> bool {
        self.last_note == other.last_note && self.last_event == other.last_event
    }
}

impl Eq for NostrRelayPool {}
impl NostrRelayPool {
    #[inline]
    #[must_use]
    pub fn relay_health(&self) -> std::collections::HashMap<String, super::ReadyState> {
        let mut health = std::collections::HashMap::new();
        for relay in self.pool.borrow().iter() {
            health.insert(relay.url.clone(), relay.ready_state);
        }
        health
    }

    /// Subscribe to notes matching a filter
    ///
    /// Returns a subscription ID that can be used to unsubscribe
    #[must_use]
    pub fn subscribe(
        &self,
        filter: nostro2::NostrSubscription,
        callback: yew::Callback<nostro2::NostrNote>,
    ) -> super::SubscriptionId {
        let id = super::SubscriptionId::new();

        let info = super::SubscriptionInfo {
            id: id.clone(),
            filter: filter.clone(),
            callback,
            created_at: nostro2::NostrNote::now(),
            note_count: 0,
        };

        // Store subscription
        self.subscriptions.borrow_mut().insert(id.clone(), info);

        // Send subscription to all relays
        self.send(filter);

        id
    }

    /// Unsubscribe from a subscription
    pub fn unsubscribe(&self, id: &super::SubscriptionId) {
        self.subscriptions.borrow_mut().remove(id);

        // Send CLOSE to relays
        self.send(nostro2::NostrClientEvent::close_subscription(id.as_str()));
    }

    /// Get all active subscriptions
    #[must_use]
    pub fn active_subscriptions(&self) -> Vec<super::SubscriptionInfo> {
        self.subscriptions.borrow().values().cloned().collect()
    }

    /// Dispatch a note to matching subscriptions
    fn dispatch_note(&self, note: &nostro2::NostrNote) {
        let mut subs = self.subscriptions.borrow_mut();

        web_sys::console::log_1(
            &format!(
                "Dispatching note kind:{} to {} subscriptions",
                note.kind,
                subs.len()
            )
            .into(),
        );

        for sub in subs.values_mut() {
            let matches = super::note_matches_filter(note, &sub.filter);
            web_sys::console::log_1(
                &format!(
                    "  Sub filter kinds:{:?} - matches: {}",
                    sub.filter.kinds, matches
                )
                .into(),
            );

            if matches {
                sub.note_count += 1;
                sub.callback.emit(note.clone());
            }
        }
    }
    #[inline]
    pub fn send<T>(&self, event: T) -> nostro2::NostrClientEvent
    where
        T: Into<nostro2::NostrClientEvent> + Clone,
    {
        let event = event.into();

        // Serialize once instead of per-relay
        let event_str = match serde_json::to_string(&event) {
            Ok(s) => s,
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to serialize event: {e}").into());
                return event;
            }
        };

        for relay in self.pool.borrow().iter() {
            match relay.ready_state {
                super::ReadyState::CONNECTING => {
                    relay.queue.borrow_mut().push(event.clone());
                }
                super::ReadyState::OPEN => {
                    if let Err(e) = relay.websocket.send_with_str(&event_str) {
                        web_sys::console::error_1(&e);
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
    NewNote(nostro2::NostrRelayEvent),
    CloseRelay(String),
    AddRelay(crate::relay_pool::UserRelay),
    RemoveRelay(crate::relay_pool::UserRelay),
}
impl Reducible for NostrRelayPool {
    type Action = NostrRelayPoolAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            NostrRelayPoolAction::AddRelay(relay) => {
                // Add to pending queue - the provider effect will handle actual connection
                self.pending_relays.borrow_mut().push(relay);
                self
            }
            NostrRelayPoolAction::RemoveRelay(relay) => {
                self.pool.borrow_mut().retain(|r| r.url != relay.url);
                // Mutated in place, return self directly
                self
            }
            NostrRelayPoolAction::Open(url) => {
                {
                    let mut pool = self.pool.borrow_mut();
                    for relay in pool.iter_mut() {
                        if relay.url == url {
                            relay.ready_state = super::ReadyState::OPEN;
                        }
                    }
                } // Drop borrow before returning self
                self
            }
            NostrRelayPoolAction::NewEvent(_event) => {
                // Don't create new state for non-note events
                // Events are handled via other mechanisms, not subscriptions
                self
            }
            NostrRelayPoolAction::NewNote(event) => {
                // Extract note from event and dispatch to subscribers
                if let nostro2::NostrRelayEvent::NewNote(.., ref note) = event {
                    self.dispatch_note(note);
                }

                // Don't create new state - dispatch_note already fired callbacks via RefCell
                // Creating new state would cause ALL components using context to re-render
                // We only want components with matching subscriptions to re-render
                self
            }
            NostrRelayPoolAction::CloseRelay(url) => {
                {
                    let mut pool = self.pool.borrow_mut();
                    for relay in pool.iter_mut() {
                        if relay.url == url {
                            relay.ready_state = super::ReadyState::CLOSED;
                        }
                    }
                } // Drop borrow before returning self
                self
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
    // Use bounded deduplication to prevent memory leaks
    // Keeps track of last 10,000 note IDs (~ 1MB max)
    let note_dedup: std::rc::Rc<std::cell::RefCell<crate::relay_pool::BoundedDedup>> =
        use_mut_ref(|| crate::relay_pool::BoundedDedup::new(10_000));

    let pending_relays = use_mut_ref(Vec::new);

    let subscriptions = use_mut_ref(std::collections::HashMap::new);

    #[allow(clippy::redundant_clone)]
    let ctx = use_reducer({
        let note_dedup = note_dedup.clone();
        let subscriptions = subscriptions.clone();
        move || NostrRelayPool {
            pool: pool.clone(),
            last_note: None,
            last_event: None,
            note_dedup,
            pending_relays,
            subscriptions,
        }
    });
    let dispatch = ctx.dispatcher();
    let pool = ctx.pool.clone();

    // Handle initial relays from props
    use_effect_with(props.relays.clone(), {
        let pool = pool.clone();
        let dispatch = dispatch.clone();
        move |relays| {
            for relay in relays {
                if pool.borrow().iter().any(|r| r.url == relay.url) {
                    continue;
                }
                let Ok(relay_ws) = super::NostrWebSocket::connect_with_retry(
                    relay.url.clone(),
                    dispatch.clone(),
                    note_dedup.clone(),
                    2,
                ) else {
                    continue;
                };
                pool.borrow_mut().push(relay_ws);
            }
        }
    });

    // Handle dynamically added relays via AddRelay action
    use_effect_with(ctx.clone(), move |ctx| {
        // Check for pending relays and connect them
        let pending_list = ctx
            .pending_relays
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        for relay in pending_list {
            if pool.borrow().iter().any(|r| r.url == relay.url) {
                continue;
            }
            let Ok(relay_ws) = super::NostrWebSocket::connect_with_retry(
                relay.url.clone(),
                dispatch.clone(),
                ctx.note_dedup.clone(),
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
