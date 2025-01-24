use crate::{
    key_manager::NostrIdProvider,
    relay_pool::{RelayPoolTest, RelayProvider, UserRelay},
};
use html::ChildrenProps;
use yew::prelude::*;
// Uncomment the following line to enable the app demo
#[wasm_bindgen_test::wasm_bindgen_test]
fn _main_app() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <AppContextProviders>
            <div class="flex flex-col h-full flex-1 gap-4 p-4 text-center items-center justify-center">
                <h1 class="text-2xl font-bold">{"Minions App Showcase"}</h1>
                // ADD NEW TEST COMPONENTS HERE WITH INLINES
                // <minions::widgets::ag_grid::NostrNotesGrid />
                // <minions::widgets::leaflet::LeafletTest />
            </div>
        </AppContextProviders>
    }
}

use wasm_bindgen::prelude::*;
use web_sys::{FetchEvent, Request, Response, ServiceWorkerGlobalScope};

#[wasm_bindgen]
pub struct AppWebWorker {
    scope: ServiceWorkerGlobalScope,
}
#[wasm_bindgen]
impl AppWebWorker {
    pub fn new() -> Self {
        let scope: ServiceWorkerGlobalScope = web_sys::js_sys::global().unchecked_into();
        Self { scope }
    }
    pub fn register_service_worker(&self) {
        // Access the global scope of the Service Worker.
        let scope = &self.scope;
        gloo::console::log!("Service Worker started!");

        // Add an event listener for the "install" event.
        let on_install_callback = Closure::wrap(Box::new(move || {
            gloo::console::log!("Service Worker installed!");
        }) as Box<dyn FnMut()>);
        scope.set_oninstall(Some(on_install_callback.as_ref().unchecked_ref()));
        on_install_callback.forget();

        // Add an event listener for the "fetch" event.
        let on_fetch_callback = Closure::wrap(Box::new(move |event: FetchEvent| {
            let request: Request = event.request();
            let url = request.url();
            log(&format!("Fetching: {}", url));

            // Respond with a simple message.
            let response = Response::new_with_opt_str(Some("Hello from Service Worker!"))
                .expect("Failed to create response");
            let res = event.respond_with(response.unchecked_ref());
            gloo::console::log!(res.is_ok());
        }) as Box<dyn FnMut(FetchEvent)>);
        scope.set_onfetch(Some(on_fetch_callback.as_ref().unchecked_ref()));
        on_fetch_callback.forget();
    }
}

// This is the entry point for the Service Worker.
#[wasm_bindgen]
pub fn register_service_worker() {
    // Access the global scope of the Service Worker.
    let scope: ServiceWorkerGlobalScope = web_sys::js_sys::global().unchecked_into();
    gloo::console::log!("Service Worker started!");

    // Add an event listener for the "install" event.
    let on_install_callback = Closure::wrap(Box::new(move || {
        gloo::console::log!("Service Worker installed!");
    }) as Box<dyn FnMut()>);
    scope.set_oninstall(Some(on_install_callback.as_ref().unchecked_ref()));
    on_install_callback.forget();

    // Add an event listener for the "fetch" event.
    let on_fetch_callback = Closure::wrap(Box::new(move |event: FetchEvent| {
        let request: Request = event.request();
        let url = request.url();
        log(&format!("Fetching: {}", url));

        // Respond with a simple message.
        let response = Response::new_with_opt_str(Some("Hello from Service Worker!"))
            .expect("Failed to create response");
        let res = event.respond_with(response.unchecked_ref());
        gloo::console::log!(res.is_ok());
    }) as Box<dyn FnMut(FetchEvent)>);
    scope.set_onfetch(Some(on_fetch_callback.as_ref().unchecked_ref()));
    on_fetch_callback.forget();
}

// Helper function to log messages to the console.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
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
