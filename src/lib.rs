pub mod browser_api;
pub mod key_manager;
pub mod relay_pool;
pub mod router;
pub mod widgets;

pub const DB_NAME: &str = "nostr_db";
pub const DB_VERSION: u32 = 2;
pub const RELAY_STORE: &str = "user_relays";
pub const RELAY_KEY: &str = "url";
pub const IDENTITY_STORE: &str = "user_identities";
pub const IDENTITY_KEY: &str = "pubkey";

#[cfg(target_arch = "wasm32")]
use browser_api::IdbStoreManager;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};

pub fn init_nostr_db() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    if let Some(idb_factory) = window.indexed_db()? {
        let idb_open_request = idb_factory.open_with_u32(DB_NAME, DB_VERSION)?;
        let on_upgrade_needed = Closure::once_into_js(move |event: web_sys::Event| {
            if let Err(e) = upgrade_nostr_db(event) {
                gloo::console::error!(&e);
            }
        });
        let on_error = Closure::once_into_js(move |event: web_sys::Event| {
            gloo::console::log!("Database error event: {:?}", event);
        });
        idb_open_request.set_onupgradeneeded(Some(on_upgrade_needed.as_ref().unchecked_ref()));
        idb_open_request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        Ok(())
    } else {
        Err(JsValue::from_str("IndexedDB not supported"))
    }
}
fn upgrade_nostr_db(event: web_sys::Event) -> Result<(), JsValue> {
    if event.target().is_none() {
        return Err(JsValue::from_str("Error upgrading database"));
    };
    let target = event.target().unwrap();
    let db = target
        .dyn_into::<web_sys::IdbOpenDbRequest>()?
        .result()?
        .dyn_into::<web_sys::IdbDatabase>()?;
    let db_store_names = db.object_store_names();
    if !db_store_names.contains(IDENTITY_STORE) {
        #[cfg(target_arch = "wasm32")]
        key_manager::UserIdentity::create_data_store(&db)?;
    }
    if !db_store_names.contains(RELAY_STORE) {
        #[cfg(target_arch = "wasm32")]
        relay_pool::UserRelay::create_data_store(&db)?;
    }
    Ok(())
}

#[cfg(test)]
mod test_app;
