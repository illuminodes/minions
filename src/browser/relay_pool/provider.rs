use yew::prelude::*;

use super::worker::RelayCommand;

/// The relay pool, as seen by the main (UI) thread.
///
/// All relay I/O — `WebSockets`, dedup, filter-matching — runs in a Web Worker
/// (see [`super::worker`]); this type is a thin **synchronous facade** over the
/// bridge to that worker. Public methods (`subscribe`, `send`, …) push a
/// [`RelayCommand`] into `cmd_tx` (fire-and-forget); the provider's driver task
/// forwards them to the worker and pumps matched notes / relay events / health
/// back, dispatching them into this same store via the reducer.
///
/// All fields are `Rc<RefCell<…>>` (or a cheap `Clone` channel), so cloning the
/// store only bumps refcounts.
#[derive(Clone)]
pub struct NostrRelayPool {
    /// Counts the changes this store has announced.
    ///
    /// Every other field is an `Rc`, so a "changed" store SHARES its interior
    /// with the old one and comparing those interiors always reports equal.
    /// `ContextProvider` uses that comparison to decide whether to notify
    /// consumers, so without a plain-value discriminator here a settled
    /// transport never reaches the UI.
    version: u64,
    /// Outbound command channel to the worker (drained by the driver task).
    cmd_tx: futures::channel::mpsc::UnboundedSender<RelayCommand>,
    /// Relays the app asked to add before the bridge existed / at runtime.
    pending_relays: std::rc::Rc<std::cell::RefCell<Vec<crate::browser::relay_pool::UserRelay>>>,
    /// Cached per-relay health, fed by `WorkerOut::RelayHealth`.
    relay_health:
        std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, super::ReadyState>>>,
    /// Which transport won, once the worker answers.
    transport: std::rc::Rc<std::cell::Cell<super::transport_status::TransportStatus>>,
    /// Note subscriptions: used to ROUTE worker-matched notes to per-hook
    /// callbacks (the worker already did the expensive matching).
    subscriptions: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<super::SubscriptionId, super::SubscriptionInfo>,
        >,
    >,
    /// Relay-event subscriptions (EOSE/OK/NOTICE/AUTH/…).
    relay_event_subscribers: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<super::SubscriptionId, super::RelayEventSubscription>,
        >,
    >,
}

// Manual Debug impl since internal details don't need to be printed.
impl std::fmt::Debug for NostrRelayPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrRelayPool")
            .field("version", &self.version)
            .field("transport", &self.transport.get())
            .field("relay_health", &self.relay_health.borrow().len())
            .field("pending_relays", &self.pending_relays.borrow().len())
            .field("subscriptions", &self.subscriptions.borrow().len())
            .field(
                "relay_event_subscribers",
                &self.relay_event_subscribers.borrow().len(),
            )
            .finish_non_exhaustive()
    }
}

// Two stores are the same render generation, or they are not. Comparing the
// `Rc` interiors cannot answer that: a changed store shares them with the old.
impl PartialEq for NostrRelayPool {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
    }
}

impl Eq for NostrRelayPool {}

impl NostrRelayPool {
    /// Create a new `Rc<Self>` that shares all interior state.
    /// This triggers a Yew re-render without cloning actual data
    /// (all fields are `Rc<RefCell<…>>` / a `Clone` channel, so only
    /// refcounts are bumped).
    fn notify_change(self: &std::rc::Rc<Self>) -> std::rc::Rc<Self> {
        let mut next = self.as_ref().clone();
        next.version = self.version.wrapping_add(1);
        std::rc::Rc::new(next)
    }

    #[must_use]
    pub fn relay_health(&self) -> std::collections::HashMap<String, super::ReadyState> {
        self.relay_health.borrow().clone()
    }

    /// Which transport carries this pool's traffic.
    ///
    /// Starts at
    /// [`Pending`](super::transport_status::TransportStatus::Pending) and
    /// settles once, when the worker answers.
    #[must_use]
    pub fn transport(&self) -> super::transport_status::TransportStatus {
        self.transport.get()
    }

    /// Subscribe to notes matching a filter.
    ///
    /// Returns a subscription ID that can be used to unsubscribe. The ID matches
    /// the REQ subscription ID the relays see, so callers can correlate EOSE
    /// responses to their subscriptions.
    #[must_use]
    pub fn subscribe(
        &self,
        filter: nostro2::NostrSubscription,
        callback: yew::Callback<nostro2::NostrNote>,
    ) -> super::SubscriptionId {
        // Build the REQ first so we can extract the relay subscription ID that
        // nostro2 generates, and return that exact ID to the caller.
        let req: nostro2::NostrClientEvent = filter.clone().into();
        let id = if let nostro2::NostrClientEvent::Subscribe(_, ref sub_id, _) = req {
            super::SubscriptionId::from_string(sub_id.clone())
        } else {
            super::SubscriptionId::new()
        };

        let encoded = (
            crate::NostrJson::to_string(&filter),
            crate::NostrJson::to_string(&req),
        );

        let info = super::SubscriptionInfo {
            id: id.clone(),
            filter,
            callback,
            created_at: crate::WallClock::unix_seconds(),
            note_count: 0,
        };
        self.subscriptions.borrow_mut().insert(id.clone(), info);

        // Hand the worker the filter (for matching), the id, and the prebuilt
        // REQ frame (so relays see the same id). Fire-and-forget.
        if let (Ok(filter_json), Ok(req_json)) = encoded {
            let _ = self.cmd_tx.unbounded_send(RelayCommand::Subscribe {
                sub_id: id.as_str().to_string(),
                filter_json,
                req_json,
            });
        }

        id
    }

    /// Unsubscribe from a subscription (sends CLOSE to relays via the worker).
    pub fn unsubscribe(&self, id: &super::SubscriptionId) {
        self.subscriptions.borrow_mut().remove(id);
        let _ = self
            .cmd_tx
            .unbounded_send(RelayCommand::Close(id.as_str().to_string()));
    }

    /// Subscribe to relay events (EOSE, OK, NOTICE, AUTH, etc.).
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

    /// Unsubscribe from relay events.
    pub fn unsubscribe_relay_events(&self, id: &super::SubscriptionId) {
        self.relay_event_subscribers.borrow_mut().remove(id);
    }

    /// Get all active subscriptions.
    #[must_use]
    pub fn active_subscriptions(&self) -> Vec<super::SubscriptionInfo> {
        self.subscriptions.borrow().values().cloned().collect()
    }

    /// Dispatch a worker-matched note to the subscriptions whose filter it
    /// matches. The worker already confirmed the note matches *some* active
    /// filter; the re-check here only ROUTES it to the right callback(s).
    fn dispatch_note(&self, note: &nostro2::NostrNote) {
        let mut subs = self.subscriptions.borrow_mut();
        for sub in subs.values_mut() {
            if sub.filter.matches(note) {
                sub.note_count += 1;
                sub.callback.emit(note.clone());
            }
        }
    }

    /// Dispatch a relay event to all relay-event subscribers.
    fn dispatch_relay_event(&self, event: &nostro2::NostrRelayEvent) {
        for sub in self.relay_event_subscribers.borrow().values() {
            sub.callback.emit(event.clone());
        }
    }

    /// Publish a client event (e.g. a signed note) to every relay via the
    /// worker. Fire-and-forget.
    pub fn send<T>(&self, event: T)
    where
        T: Into<nostro2::NostrClientEvent>,
    {
        let event: nostro2::NostrClientEvent = event.into();
        if let Ok(event_json) = crate::NostrJson::to_string(&event) {
            let _ = self.cmd_tx.unbounded_send(RelayCommand::Send(event_json));
        }
    }
}

/// Actions that drive the relay-pool store.
///
/// `WorkerEvent` carries a [`WorkerOut`] pumped off the bridge by the driver
/// task. Health updates re-render consumers (`notify_change`); note / relay-event
/// dispatch returns `self` (no global re-render — fan-out is per-subscription).
/// `AddRelay` / `RemoveRelay` enqueue a worker command and re-render.
pub enum NostrRelayPoolAction {
    WorkerEvent(super::pool_event::PoolEvent),
    AddRelay(crate::browser::relay_pool::UserRelay),
    RemoveRelay(crate::browser::relay_pool::UserRelay),
    TransportSettled(super::transport_status::TransportStatus),
}

impl Reducible for NostrRelayPool {
    type Action = NostrRelayPoolAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            NostrRelayPoolAction::AddRelay(relay) => {
                let _ = self
                    .cmd_tx
                    .unbounded_send(RelayCommand::AddRelay(relay.url.clone()));
                self.pending_relays.borrow_mut().push(relay);
                self.notify_change()
            }
            NostrRelayPoolAction::RemoveRelay(relay) => {
                let _ = self
                    .cmd_tx
                    .unbounded_send(RelayCommand::RemoveRelay(relay.url.clone()));
                self.relay_health.borrow_mut().remove(&relay.url);
                self.notify_change()
            }
            NostrRelayPoolAction::TransportSettled(status) => {
                if self.transport.get() == status {
                    return self;
                }
                self.transport.set(status);
                self.notify_change()
            }
            NostrRelayPoolAction::WorkerEvent(out) => match out {
                super::pool_event::PoolEvent::Note(note) => {
                    self.dispatch_note(&note);
                    self
                }
                super::pool_event::PoolEvent::RelayEvent(event) => {
                    self.dispatch_relay_event(&event);
                    self
                }
                super::pool_event::PoolEvent::RelayHealth(updates) => {
                    {
                        let mut health = self.relay_health.borrow_mut();
                        for (url, state) in updates {
                            health.insert(url, state);
                        }
                    }
                    self.notify_change()
                }
            },
        }
    }
}

pub type NostrRelayPoolStore = UseReducerHandle<NostrRelayPool>;

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct RelayContextProps {
    pub children: Children,
    pub relays: Vec<crate::browser::relay_pool::UserRelay>,
}

/// Hold the transport status across renders.
///
/// A `Cell` and not a `RefCell` because the status is `Copy` and is written
/// once; a `Cell` cannot panic on a borrow conflict.
#[hook]
fn use_transport_cell() -> std::rc::Rc<std::cell::Cell<super::transport_status::TransportStatus>> {
    (*use_state(|| {
        std::rc::Rc::new(std::cell::Cell::new(
            super::transport_status::TransportStatus::default(),
        ))
    }))
    .clone()
}

#[function_component(NostrRelayPoolProvider)]
pub fn nostr_relay_pool_provider(props: &RelayContextProps) -> Html {
    let pending_relays = use_mut_ref(Vec::new);
    let relay_health = use_mut_ref(std::collections::HashMap::new);
    let transport = use_transport_cell();
    let subscriptions = use_mut_ref(std::collections::HashMap::new);
    let relay_event_subscribers = use_mut_ref(std::collections::HashMap::new);

    // The outbound command channel. The sender lives in the store; the receiver
    // is handed to the driver task on mount.
    let cmd_channel = use_mut_ref(|| Some(futures::channel::mpsc::unbounded::<RelayCommand>()));

    let ctx = use_reducer({
        let cmd_channel = cmd_channel.clone();
        move || {
            // Take the sender now; the receiver stays in `cmd_channel` for the
            // mount effect to claim.
            let cmd_tx = cmd_channel
                .borrow()
                .as_ref()
                .expect("cmd channel initialized")
                .0
                .clone();
            NostrRelayPool {
                version: 0,
                cmd_tx,
                pending_relays,
                relay_health,
                transport,
                subscriptions,
                relay_event_subscribers,
            }
        }
    });

    // Spawn the worker + driver ONCE on mount. The driver owns the bridge and
    // the command receiver: it forwards commands to the worker and pumps worker
    // output back into the store via the reducer.
    use_effect_with((), {
        let dispatch = ctx.dispatcher();
        let cmd_channel = cmd_channel;
        let relays = props.relays.clone();
        move |()| {
            // Claim the receiver (only present on the first mount).
            let Some((_, cmd_rx)) = cmd_channel.borrow_mut().take() else {
                return;
            };
            match super::worker_boot::spawn_relay_bridge() {
                Ok(bridge) => spawn_driver(bridge, cmd_rx, dispatch, relays),
                Err(e) => web_sys::console::error_1(
                    &format!("relay pool: failed to spawn worker: {e:?}").into(),
                ),
            }
        }
    });

    // Forward dynamically added relays (AddRelay action queues a worker command
    // in the reducer; this effect is kept so pending_relays stays observable for
    // consumers that inspect it).
    use_effect_with(ctx.clone(), move |_ctx| ());

    html! {
        <ContextProvider<NostrRelayPoolStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<NostrRelayPoolStore>>
    }
}

/// Drive the bridge: forward outbound commands to the worker and pump worker
/// output into the store. Runs for the lifetime of the page.
fn spawn_driver(
    bridge: yew_agent::reactor::ReactorBridge<super::worker::RelayReactor>,
    mut cmd_rx: futures::channel::mpsc::UnboundedReceiver<RelayCommand>,
    dispatch: yew::UseReducerDispatcher<NostrRelayPool>,
    relays: Vec<crate::browser::relay_pool::UserRelay>,
) {
    use futures::StreamExt;

    yew::platform::spawn_local(async move {
        let mut link = super::app_link::AppLink::pending();

        // Initial connect: hand the worker every relay URL from props. This
        // goes over the bridge because the worker has not offered rings yet;
        // the worker acts on it either way.
        let urls: Vec<String> = relays.into_iter().map(|r| r.url).collect();
        if !urls.is_empty() {
            bridge.send_input(RelayCommand::Connect(urls));
        }

        // Once the rings carry the notes, the worker stops sending bridge
        // frames. Without a wake-up of its own this loop would block on
        // `select!` and never read the ring it just adopted.
        let (tick_tx, mut tick_rx) = futures::channel::mpsc::unbounded();
        let _pump = super::ring_pump::RingPump::start(tick_tx);

        let mut bridge = bridge;
        let mut negotiation = super::transport_negotiation::TransportNegotiation::new();
        let settle = |status, link: &super::app_link::AppLink| {
            web_sys::console::log_1(&link.describe().into());
            dispatch.dispatch(NostrRelayPoolAction::TransportSettled(status));
        };
        loop {
            futures::select! {
                cmd = cmd_rx.next() => match cmd {
                    Some(cmd) => {
                        // One transport carries it, never both.
                        if !link.send(&cmd) {
                            bridge.send_input(cmd);
                        }
                    }
                    None => break, // store dropped
                },
                out = bridge.next() => match out {
                    Some(out) => {
                        if link.adopt_offered_rings() {
                            if let Some(status) = negotiation.rings_adopted() {
                                settle(status, &link);
                            }
                        } else if !link.uses_shared_rings() {
                            if let Some(status) = negotiation.bridge_frame() {
                                settle(status, &link);
                            }
                        }
                        if let Some(event) = super::pool_event::PoolEvent::from_bridge(out) {
                            dispatch.dispatch(NostrRelayPoolAction::WorkerEvent(event));
                        }
                    }
                    None => break, // worker gone
                },
                tick = tick_rx.next() => {
                    if tick.is_none() {
                        break; // pump stopped
                    }
                    // The handshake is consumed by `Codec::decode` and returns
                    // `WorkerLoaded`, which `yew-agent` swallows internally. It
                    // therefore never reaches the bridge arm above, so the
                    // offer must be adopted here or it is never adopted at all.
                    if link.adopt_offered_rings() {
                        if let Some(status) = negotiation.rings_adopted() {
                            settle(status, &link);
                        }
                    } else if let Some(status) = negotiation.tick_without_offer() {
                        settle(status, &link);
                    }
                },
            }

            for event in link.drain() {
                dispatch.dispatch(NostrRelayPoolAction::WorkerEvent(event));
            }
        }
    });
}
