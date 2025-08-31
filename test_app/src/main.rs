use nostr_minions::NostrAppProvider;
use yew::prelude::*;

#[wasm_bindgen_test::wasm_bindgen_test]
pub fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    let relays = vec![
        nostr_minions::UserRelay {
            url: "wss://relay.illuminodes.com".to_string(),
            read: true,
            write: true,
        },
        nostr_minions::UserRelay {
            url: "wss://relay.arrakis.lat".to_string(),
            read: true,
            write: true,
        },
    ];
    html! {
        <NostrAppProvider {relays}>
            <div class="h-dvw w-dvw items-center justify-center flex flex-col overflow-y-auto">
                <h1 class="text-2xl font-bold">{"Minions App Showcase"}</h1>
                // ADD NEW TEST COMPONENTS HERE WITH INLINES
                <nostr_minions::RelayPoolTest />
                <nostr_minions::NostrIdLoginTest />
            </div>
        </NostrAppProvider>
    }
}

