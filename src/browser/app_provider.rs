use crate::{IdbManagerProvider, NostrIdProvider, NostrRelayPoolProvider, UserRelay};

#[derive(Clone, Debug, PartialEq, yew::Properties)]
pub struct AppProps {
    pub children: yew::html::Children,
    #[prop_or_default]
    pub relays: Vec<UserRelay>,
    #[prop_or_default]
    pub fallback: yew::html::Html,
}

#[yew::function_component(NostrAppProvider)]
pub fn nostr_app_provider(props: &AppProps) -> yew::Html {
    yew::html! {
        <yew::suspense::Suspense fallback={props.fallback.clone()}>
            <IdbManagerProvider>
                <NostrRelayPoolProvider relays={props.relays.clone()}>
                    <NostrIdProvider>
                        {props.children.clone()}
                    </NostrIdProvider>
                </NostrRelayPoolProvider>
            </IdbManagerProvider>
        </yew::suspense::Suspense>
    }
}
