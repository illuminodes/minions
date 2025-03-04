use nostr_minions::{
    key_manager::NostrIdProvider,
    relay_pool::{RelayProvider, UserRelay},
};
use html::ChildrenProps;
use yew::prelude::*;

#[wasm_bindgen_test::wasm_bindgen_test]
pub fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <AppContextProviders>
            <div class="h-dvw w-dvw items-center justify-center flex flex-col overflow-y-auto">
                <h1 class="text-2xl font-bold">{"Minions App Showcase"}</h1>
                // ADD NEW TEST COMPONENTS HERE WITH INLINES
                <nostr_minions::widgets::ag_grid::NostrNotesGrid />
                <nostr_minions::widgets::leaflet::LeafletTest />
                <nostr_minions::widgets::full_calendar::FullCalendarTest/>
                <nostr_minions::key_manager::NostrIdLoginTest />
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
