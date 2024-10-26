use html::ChildrenProps;
use minions::{
    key_manager::key_manager::NostrIdProvider,
    relay_pool::{nostr_relay::UserRelay, relay_pool::RelayProvider, RelayPoolTest},
};
use yew::prelude::*;
use minions::widgets::ag_grid::NostrNotesGrid;

fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <AppContextProviders>
            <div class="flex flex-col h-full flex-1 gap-4 p-4 text-center items-center justify-center">
                <h1 class="text-2xl font-bold">{"Minions App Showcase"}</h1>
                <RelayPoolTest />
                <NostrNotesGrid />
            </div>
        </AppContextProviders>
    }
}

#[function_component(AppContextProviders)]
fn app_context_providers(props: &ChildrenProps) -> Html {
    let relays = vec![
        UserRelay {
            url: "wss://relay.illuminodes.com".to_string(),
            read: true,
            write: true,
        },
        UserRelay {
            url: "wss://relay.arrakis.lat".to_string(),
            read: true,
            write: true,
        },
    ];
    html! {
        <RelayProvider {relays} >
            <NostrIdProvider>
                {props.children.clone()}
            </NostrIdProvider>
        </RelayProvider>
    }
}
