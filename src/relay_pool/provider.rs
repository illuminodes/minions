use std::rc::Rc;
use yew::{platform::spawn_local, prelude::*};

#[derive(Clone, Debug, PartialEq)]
pub struct NostrRelayPool {
    pool: nostro2_web_relay::pool::RelayPool,
    pub unique_notes: Vec<crate::nostro2::note::NostrNote>,
    pub relay_events: Vec<crate::nostro2::relay_events::NostrRelayEvent>,
}
impl NostrRelayPool {
    pub fn send<T>(&self, msg: T) -> crate::nostro2::relay_events::NostrClientEvent
    where
        T: Into<crate::nostro2::relay_events::NostrClientEvent>
            + Send
            + 'static
            + Sync
            + Clone
            + std::fmt::Debug,
    {
        let msg_res = msg.clone();
        let pool_clone = self.pool.clone();
        yew::platform::spawn_local(async move {
            pool_clone.send(msg_res).await;
        });
        msg.into()
    }
    pub async fn relay_pool_status(&self) -> Vec<crate::nostro2::relay_events::RelayStatus> {
        self.pool.status().await
    }
}

pub enum NostrRelayPoolAction {
    NewNote(crate::nostro2::note::NostrNote),
    NewRelayEvent(crate::nostro2::relay_events::NostrRelayEvent),
}
impl Reducible for NostrRelayPool {
    type Action = NostrRelayPoolAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            NostrRelayPoolAction::NewNote(note) => {
                let mut relay_pool: NostrRelayPool = (*self.clone()).clone();
                relay_pool.unique_notes.push(note);
                relay_pool.into()
            }
            NostrRelayPoolAction::NewRelayEvent(event) => {
                let mut relay_pool: NostrRelayPool = (*self.clone()).clone();
                relay_pool.relay_events.push(event);
                relay_pool.into()
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
    let ctx = use_reducer(|| NostrRelayPool {
        pool: nostro2_web_relay::pool::RelayPool::from(
            props
                .relays
                .iter()
                .map(|r| r.url.to_string())
                .collect::<Vec<String>>()
                .as_slice(),
        ),
        unique_notes: vec![],
        relay_events: vec![],
    });
    let ctx_clone = ctx.clone();
    use_memo((), move |_| {
        spawn_local(async move {
            while let Some(msg) = ctx_clone.pool.read().await {
                match msg {
                    crate::nostro2::relay_events::NostrRelayEvent::NewNote(.., note) => {
                        ctx_clone.dispatch(NostrRelayPoolAction::NewNote(note));
                    }
                    event => {
                        ctx_clone.dispatch(NostrRelayPoolAction::NewRelayEvent(event));
                    }
                }
            }
        });
    });

    // use_effect_with((), |_| || {});

    html! {
        <ContextProvider<NostrRelayPoolStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<NostrRelayPoolStore>>
    }
}
