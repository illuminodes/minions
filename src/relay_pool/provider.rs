use yew::prelude::*;

#[derive(Clone)]
pub struct NostrRelayPool {
    pool: std::rc::Rc<std::cell::RefCell<Vec<super::NostrWebSocket>>>,
    // Store dedup tracker and pending relays for dynamic relay addition
    note_dedup: std::rc::Rc<std::cell::RefCell<super::BoundedDedup>>,
    pending_relays: std::rc::Rc<std::cell::RefCell<Vec<crate::relay_pool::UserRelay>>>,
    // Note subscription management
    subscriptions: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<super::SubscriptionId, super::SubscriptionInfo>,
        >,
    >,
    // Relay event subscription management
    relay_event_subscribers: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<super::SubscriptionId, super::RelayEventSubscription>,
        >,
    >,
}

// Manual Debug impl since internal details don't need to be printed
impl std::fmt::Debug for NostrRelayPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrRelayPool")
            .field("pool_size", &self.pool.borrow().len())
            .field("note_dedup_size", &self.note_dedup.borrow().len())
            .field("pending_relays", &self.pending_relays.borrow().len())
            .field("subscriptions", &self.subscriptions.borrow().len())
            .field(
                "relay_event_subscribers",
                &self.relay_event_subscribers.borrow().len(),
            )
            .finish()
    }
}

// Compare observable pool state: relay URLs + ready states + subscription count
impl PartialEq for NostrRelayPool {
    fn eq(&self, other: &Self) -> bool {
        let self_pool = self.pool.borrow();
        let other_pool = other.pool.borrow();
        if self_pool.len() != other_pool.len() {
            return false;
        }
        let self_health: Vec<_> = self_pool.iter().map(|r| (&r.url, r.ready_state)).collect();
        let other_health: Vec<_> = other_pool.iter().map(|r| (&r.url, r.ready_state)).collect();
        self_health == other_health
            && self.subscriptions.borrow().len() == other.subscriptions.borrow().len()
    }
}

impl Eq for NostrRelayPool {}

impl NostrRelayPool {
    /// Create a new `Rc<Self>` that shares all interior state.
    /// This triggers a Yew re-render without cloning actual data
    /// (all fields are `Rc<RefCell<…>>` so only refcounts are bumped).
    fn notify_change(self: &std::rc::Rc<Self>) -> std::rc::Rc<Self> {
        std::rc::Rc::new(Self {
            pool: self.pool.clone(),
            note_dedup: self.note_dedup.clone(),
            pending_relays: self.pending_relays.clone(),
            subscriptions: self.subscriptions.clone(),
            relay_event_subscribers: self.relay_event_subscribers.clone(),
        })
    }

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

    /// Subscribe to relay events (EOSE, OK, NOTICE, AUTH, etc.)
    ///
    /// Returns a subscription ID that can be used to unsubscribe.
    /// Note events are excluded — use `subscribe` for those.
    #[must_use]
    pub fn subscribe_relay_events(
        &self,
        callback: yew::Callback<nostro2::NostrRelayEvent>,
    ) -> super::SubscriptionId {
        let id = super::SubscriptionId::new();

        let sub = super::RelayEventSubscription {
            id: id.clone(),
            callback,
        };

        self.relay_event_subscribers
            .borrow_mut()
            .insert(id.clone(), sub);

        id
    }

    /// Unsubscribe from relay events
    pub fn unsubscribe_relay_events(&self, id: &super::SubscriptionId) {
        self.relay_event_subscribers.borrow_mut().remove(id);
    }

    /// Get all active subscriptions
    #[must_use]
    pub fn active_subscriptions(&self) -> Vec<super::SubscriptionInfo> {
        self.subscriptions.borrow().values().cloned().collect()
    }

    /// Dispatch a note to matching subscriptions
    fn dispatch_note(&self, note: &nostro2::NostrNote) {
        let mut subs = self.subscriptions.borrow_mut();

        for sub in subs.values_mut() {
            let matches = super::note_matches_filter(note, &sub.filter);

            if matches {
                sub.note_count += 1;
                sub.callback.emit(note.clone());
            }
        }
    }

    /// Dispatch a relay event to all relay event subscribers
    fn dispatch_relay_event(&self, event: &nostro2::NostrRelayEvent) {
        let subs = self.relay_event_subscribers.borrow();
        for sub in subs.values() {
            sub.callback.emit(event.clone());
        }
    }

    pub fn send<T>(&self, event: T) -> nostro2::NostrClientEvent
    where
        T: Into<nostro2::NostrClientEvent> + Clone,
    {
        let event = event.into();

        // Serialize once instead of per-relay
        let Ok(event_str) = serde_json::to_string(&event) else {
            return event;
        };

        for relay in self.pool.borrow().iter() {
            // Use actual WebSocket state to avoid race between onopen
            // firing and the reducer dispatch updating the cached field.
            match relay.actual_ready_state() {
                super::ReadyState::CONNECTING => {
                    relay.queue.borrow_mut().push(event.clone());
                }
                super::ReadyState::OPEN => {
                    let _ = relay.websocket().send_with_str(&event_str);
                }
                _ => {}
            }
        }
        event
    }
}

pub enum NostrRelayPoolAction {
    Open(String),
    RelayEvent(nostro2::NostrRelayEvent),
    CloseRelay(String),
    AddRelay(crate::relay_pool::UserRelay),
    RemoveRelay(crate::relay_pool::UserRelay),
    Reconnected(super::NostrWebSocket),
}

/// `Reducible` impl for the relay pool.
///
/// Actions that change pool composition or health (`Open`, `CloseRelay`, `Reconnected`,
/// `AddRelay`, `RemoveRelay`) return a new `Rc<Self>` so the context re-renders consumers.
///
/// `RelayEvent` intentionally returns `self` (no re-render) — event dispatch is handled
/// by per-subscription callbacks, avoiding global re-renders.
impl Reducible for NostrRelayPool {
    type Action = NostrRelayPoolAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            NostrRelayPoolAction::AddRelay(relay) => {
                self.pending_relays.borrow_mut().push(relay);
                self.notify_change()
            }
            NostrRelayPoolAction::RemoveRelay(relay) => {
                self.pool.borrow_mut().retain(|r| r.url != relay.url);
                self.notify_change()
            }
            NostrRelayPoolAction::Open(url) => {
                {
                    let mut pool = self.pool.borrow_mut();
                    for relay in pool.iter_mut() {
                        if relay.url == url {
                            relay.ready_state = super::ReadyState::OPEN;
                        }
                    }
                }
                self.notify_change()
            }
            NostrRelayPoolAction::RelayEvent(ref event) => {
                if let nostro2::NostrRelayEvent::NewNote(.., ref note) = event {
                    self.dispatch_note(note);
                } else {
                    self.dispatch_relay_event(event);
                }
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
                }
                self.notify_change()
            }
            NostrRelayPoolAction::Reconnected(ws) => {
                {
                    let mut pool = self.pool.borrow_mut();
                    pool.retain(|r| r.url != ws.url);
                    pool.push(ws);
                }
                self.notify_change()
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
pub fn nostr_relay_pool_provider(props: &RelayContextProps) -> Html {
    let pool = use_mut_ref(Vec::new);
    // Use bounded deduplication to prevent memory leaks
    // Keeps track of last 10,000 note IDs (~ 1MB max)
    let note_dedup: std::rc::Rc<std::cell::RefCell<crate::relay_pool::BoundedDedup>> =
        use_mut_ref(|| crate::relay_pool::BoundedDedup::new(10_000));

    let pending_relays = use_mut_ref(Vec::new);

    let subscriptions = use_mut_ref(std::collections::HashMap::new);

    let relay_event_subscribers = use_mut_ref(std::collections::HashMap::new);

    #[allow(clippy::redundant_clone)]
    let ctx = use_reducer({
        let note_dedup = note_dedup.clone();
        let subscriptions = subscriptions.clone();
        let relay_event_subscribers = relay_event_subscribers.clone();
        move || NostrRelayPool {
            pool: pool.clone(),
            note_dedup,
            pending_relays,
            subscriptions,
            relay_event_subscribers,
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
                let Ok(relay_ws) = super::NostrWebSocket::connect(
                    relay.url.clone(),
                    dispatch.clone(),
                    note_dedup.clone(),
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
            let Ok(relay_ws) = super::NostrWebSocket::connect(
                relay.url.clone(),
                dispatch.clone(),
                ctx.note_dedup.clone(),
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
