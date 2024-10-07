use wasm_bindgen::prelude::*;
use web_sys::{ServiceWorkerGlobalScope, FetchEvent, PushEvent, PushSubscription};
use js_sys::{Promise, Array, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use nostro2::{Client, ClientBuilder, Event, Filter, Subscription};
use serde_json::json;

const CACHE_NAME: &str = "minions-cache-v1";
const ASSETS_TO_CACHE: &[&str] = &[
    "/",
    "/index.html",
    "/minions_bg.wasm",
    "/minions.js",
    // Add other assets you want to cache
];

#[wasm_bindgen]
pub fn init_service_worker() {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();

    global.add_event_listener_with_callback("install", Closure::wrap(Box::new(install_handler) as Box<dyn Fn()>).into_js_value().unchecked_ref()).unwrap();
    global.add_event_listener_with_callback("activate", Closure::wrap(Box::new(activate_handler) as Box<dyn Fn()>).into_js_value().unchecked_ref()).unwrap();
    global.add_event_listener_with_callback("fetch", Closure::wrap(Box::new(fetch_handler) as Box<dyn Fn(FetchEvent)>).into_js_value().unchecked_ref()).unwrap();
    global.add_event_listener_with_callback("message", Closure::wrap(Box::new(message_handler) as Box<dyn Fn(web_sys::MessageEvent)>).into_js_value().unchecked_ref()).unwrap();
    global.add_event_listener_with_callback("push", Closure::wrap(Box::new(push_handler) as Box<dyn Fn(PushEvent)>).into_js_value().unchecked_ref()).unwrap();

    subscribe_to_nostr_events();
}

fn install_handler() {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    
    let future = async move {
        let cache = global.caches().open(CACHE_NAME).await.unwrap();
        let _ = cache.add_all(&ASSETS_TO_CACHE.iter().map(|&s| JsValue::from_str(s)).collect::<js_sys::Array>()).await;
    };

    global.skip_waiting();
    global.extend_with_event_listener_options(future.into_js_value().unchecked_ref());
}

fn activate_handler() {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    
    let future = async move {
        let cache_keys = global.caches().keys().await.unwrap();
        for i in 0..cache_keys.length() {
            if let Some(key) = cache_keys.get(i) {
                if key.as_string().unwrap() != CACHE_NAME {
                    let _ = global.caches().delete(&key).await;
                }
            }
        }
    };

    global.clients().claim();
    global.extend_with_event_listener_options(future.into_js_value().unchecked_ref());
}

fn fetch_handler(event: FetchEvent) {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    let request = event.request();
    let cache_promise = global.caches().match_(&request);

    let future = async move {
        if let Ok(response) = JsFuture::from(cache_promise).await {
            if !response.is_undefined() {
                return response.unchecked_into();
            }
        }

        let fetch_promise: Promise = global.fetch_with_request(&request);
        let response = JsFuture::from(fetch_promise).await.unwrap();
        
        // Cache the response
        let cache = global.caches().open("v1").await.unwrap();
        let _ = cache.put_with_request(&request, &response.clone().unchecked_into()).await;

        response.unchecked_into()
    };

    event.respond_with(&future.into_js_value()).unwrap();
}

fn message_handler(event: web_sys::MessageEvent) {
    if let Ok(data) = event.data().dyn_into::<js_sys::Object>() {
        if let Some(event_type) = js_sys::Reflect::get(&data, &"type".into()).ok() {
            if event_type == "nostr-event" {
                if let Some(nostr_event) = js_sys::Reflect::get(&data, &"event".into()).ok() {
                    handle_nostr_event(nostr_event);
                }
            }
        }
    }
}

fn handle_nostr_event(event: JsValue) {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    
    // Parse the event data and create a notification
    // This is a simplified example; you'll need to adjust based on your event structure
    if let Some(content) = js_sys::Reflect::get(&event, &"content".into()).ok() {
        let content_str = content.as_string().unwrap_or_default();
        let _ = global.registration().show_notification("New Nostr Event", 
            web_sys::NotificationOptions::new().body(&content_str));
    }
}

fn push_handler(event: PushEvent) {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    let data = event.data().unwrap().text().unwrap();
    
    let show_notification_promise = global.registration().show_notification("New Nostr Event", 
        web_sys::NotificationOptions::new().body(&data));
    
    event.wait_until(&show_notification_promise);
}

fn subscribe_to_nostr_events() {
    wasm_bindgen_futures::spawn_local(async {
        let client = ClientBuilder::default()
            .add_relay("wss://relay.damus.io")
            .build()
            .await
            .unwrap();

        let subscription = client.subscribe(vec![
            Filter::new().kind(1), // Subscribe to text notes
        ]).await;

        while let Ok(event) = subscription.recv().await {
            handle_nostr_event(event);
        }
    });
}

#[wasm_bindgen]
pub async fn request_push_permission() -> Result<(), JsValue> {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    let permission = global.registration().push_manager().subscribe(web_sys::PushSubscriptionOptionsInit::new()).await?;
    
    // Send the subscription to your server
    send_subscription_to_server(permission).await?;
    
    Ok(())
}

async fn send_subscription_to_server(subscription: PushSubscription) -> Result<(), JsValue> {
    let global: ServiceWorkerGlobalScope = js_sys::global().unchecked_into();
    
    let json = subscription.to_json()?;
    let body = Uint8Array::from(json.as_bytes());
    
    let mut init = web_sys::RequestInit::new();
    init.method("POST")
        .body(Some(&body.buffer()));
    
    let request = web_sys::Request::new_with_str_and_init("https://your-server.com/push-subscription", &init)?;
    request.headers().set("Content-Type", "application/json")?;
    
    let response = JsFuture::from(global.fetch_with_request(&request)).await?;
    let response: web_sys::Response = response.dyn_into()?;
    
    if !response.ok() {
        return Err(JsValue::from_str("Failed to send subscription to server"));
    }
    
    Ok(())
}