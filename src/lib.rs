#![warn(
    clippy::all,
    clippy::missing_errors_doc,
    clippy::style,
    clippy::unseparated_literal_suffix,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::future_not_send, clippy::missing_errors_doc)]

pub mod browser_api;
pub mod constants;
pub mod key_manager;
pub mod relay_pool;
pub mod widgets;
pub use nostro2_signer::nostro2;
pub extern crate nostro2_signer;

pub const DB_NAME: &str = "nostr_db";
pub const DB_VERSION: u32 = 3;
pub const RELAY_STORE: &str = "user_relays";
pub const RELAY_KEY: &str = "url";
pub const IDENTITY_STORE: &str = "user_identities";
pub const IDENTITY_KEY: &str = "pubkey";

use browser_api::IdbStoreManager;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};

/// Initializes the `IndexedDB` database for the `Nostr` application.
///
/// # Errors
///
/// Returns an error if the database cannot be opened or if the upgrade fails.
pub fn init_nostr_db() -> Result<(), JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No global `window` exists, which is unexpected"))?;
    if let Some(idb_factory) = window.indexed_db()? {
        let idb_open_request = idb_factory.open_with_u32(DB_NAME, DB_VERSION)?;
        let on_upgrade_needed = Closure::once_into_js(move |event: web_sys::Event| {
            if let Err(e) = upgrade_nostr_db(&event) {
                gloo::console::error!(&e);
            }
        });
        let on_error = Closure::once_into_js(move |event: web_sys::Event| {
            gloo::console::log!(format!("Database error event: {event:?}"));
        });
        idb_open_request.set_onupgradeneeded(Some(on_upgrade_needed.as_ref().unchecked_ref()));
        idb_open_request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        Ok(())
    } else {
        Err(JsValue::from_str("IndexedDB not supported"))
    }
}
fn upgrade_nostr_db(event: &web_sys::Event) -> Result<(), JsValue> {
    if event.target().is_none() {
        return Err(JsValue::from_str("Error upgrading database"));
    }
    let target = event.target().unwrap();
    let db = target
        .dyn_into::<web_sys::IdbOpenDbRequest>()?
        .result()?
        .dyn_into::<web_sys::IdbDatabase>()?;
    let db_store_names = db.object_store_names();
    if !db_store_names.contains(IDENTITY_STORE) {
        key_manager::UserIdentity::create_data_store(&db)?;
    }
    if !db_store_names.contains(RELAY_STORE) {
        relay_pool::UserRelay::create_data_store(&db)?;
    }
    if !db_store_names.contains("sync_store") {
        LastSyncTime::create_data_store(&db)?;
    }
    Ok(())
}

#[yew::hook]
pub fn use_last_sync_time() -> Option<i64> {
    let sync_time = yew::suspense::use_future(|| async { LastSyncTime::find().await }).ok()?;
    Some(*sync_time)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct LastSyncTime {
    id: String,
    timestamp: i64,
}
impl LastSyncTime {
    pub async fn find() -> i64 {
        let Ok(last_sync_time) = Self::retrieve_from_store::<Self>(&"last_sync_time".into()).await
        else {
            return 0;
        };
        last_sync_time.timestamp
    }
    pub async fn new_sync_time() -> Result<(), JsValue> {
        Self::default().save_to_store().await
    }
}
impl TryFrom<JsValue> for LastSyncTime {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}
impl From<LastSyncTime> for JsValue {
    fn from(val: LastSyncTime) -> Self {
        serde_wasm_bindgen::to_value(&val).unwrap_or_default()
    }
}
impl Default for LastSyncTime {
    fn default() -> Self {
        Self {
            id: "last_sync_time".to_string(),
            #[allow(clippy::cast_possible_truncation)]
            timestamp: web_sys::js_sys::Date::now() as i64,
        }
    }
}
impl IdbStoreManager for LastSyncTime {
    fn config() -> crate::browser_api::IdbStoreConfig {
        crate::browser_api::IdbStoreConfig {
            db_version: DB_VERSION,
            db_name: DB_NAME,
            store_name: "sync_store",
            document_key: "id",
        }
    }
    fn key(&self) -> JsValue {
        JsValue::from_str("id")
    }
}

