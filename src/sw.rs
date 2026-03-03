use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ServiceWorkerGlobalScope, ExtendableEvent, FetchEvent};
use js_sys::{Array, Promise};
use wasm_bindgen_futures::JsFuture;

const CACHE_NAME: &str = "minions-cache-v1";
const ASSETS: &[&str] = &[
    "/index.html",
    "/styles.css",
    "/main.wasm",
];

#[wasm_bindgen(start)]
pub fn main() {
    let global = js_sys::global().unchecked_into::<ServiceWorkerGlobalScope>();

    // Install event: cache assets
    let on_install = Closure::wrap(Box::new(move |event: ExtendableEvent| {
        let caches = global.caches();
        let open_promise = caches.open(CACHE_NAME);
        let cache_assets = JsFuture::from(open_promise).then(&Closure::once_into_js(move |cache_js| {
            let cache: web_sys::Cache = cache_js.unwrap().unchecked_into();
            let mut arr = Array::new();
            for &asset in ASSETS {
                let req = web_sys::Request::new_with_str(asset).unwrap();
                let p: Promise = cache.add_with_request(&req).unwrap();
                arr.push(&p);
            }
            Promise::all(&arr)
        }));
        event.wait_until(&cache_assets);
    }) as Box<dyn FnMut(_)>);
    global.add_event_listener_with_callback("install", on_install.as_ref().unchecked_ref()).unwrap();
    on_install.forget();

    // Activate event: cleanup old caches
    let on_activate = Closure::wrap(Box::new(move |event: ExtendableEvent| {
        let caches = global.caches();
        let keys_promise = JsFuture::from(caches.keys());
        let cleanup = keys_promise.then(&Closure::once_into_js(move |keys_js| {
            let keys: Array = keys_js.unwrap().unchecked_into();
            let mut arr = Array::new();
            for key in keys.iter() {
                if key.as_string().as_deref() != Some(CACHE_NAME) {
                    arr.push(&caches.delete(&key.as_string().unwrap()).unwrap());
                }
            }
            Promise::all(&arr)
        }));
        event.wait_until(&cleanup);
    }) as Box<dyn FnMut(_)>);
    global.add_event_listener_with_callback("activate", on_activate.as_ref().unchecked_ref()).unwrap();
    on_activate.forget();

    // Fetch event: respond from cache or network
    let on_fetch = Closure::wrap(Box::new(move |event: FetchEvent| {
        let request = event.request();
        let match_promise = JsFuture::from(global.caches().match_with_request(&request));
        let response = match_promise.then(&Closure::once_into_js(move |maybe_js| {
            if let Ok(maybe) = maybe_js {
                if !maybe.is_undefined() {
                    return Promise::resolve(&maybe);
                }
            }
            global.fetch_with_request(&request)
        }));
        event.respond_with(&response).unwrap();
    }) as Box<dyn FnMut(_)>);
    global.add_event_listener_with_callback("fetch", on_fetch.as_ref().unchecked_ref()).unwrap();
    on_fetch.forget();
}