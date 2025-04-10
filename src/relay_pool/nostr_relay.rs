use gloo::utils::format::JsValueSerdeExt;
use web_sys::wasm_bindgen::JsValue;

use crate::{browser_api::IdbStoreManager, DB_NAME, DB_VERSION, RELAY_KEY, RELAY_STORE};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UserRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
impl TryFrom<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        value.into_serde().map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
impl From<UserRelay> for JsValue {
    fn from(val: UserRelay) -> Self {
        JsValue::from_serde(&val).unwrap()
    }
}
impl IdbStoreManager for UserRelay {
    fn config() -> crate::browser_api::IdbStoreConfig {
        crate::browser_api::IdbStoreConfig {
            db_version: DB_VERSION,
            db_name: DB_NAME,
            store_name: RELAY_STORE,
            document_key: RELAY_KEY,
        }
    }
    fn key(&self) -> JsValue {
        JsValue::from_str(&self.url)
    }
}

#[cfg(test)]
mod tests {
    use crate::init_nostr_db;

    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn _relay_idb_manager() -> Result<(), JsValue> {
        init_nostr_db().expect("Error initializing db");
        let user_relay = UserRelay {
            url: "wss://example.com".to_string(),
            read: true,
            write: false,
        };
        user_relay
            .save_to_store()
            .await.expect("Error saving to store");
        let retrieved: UserRelay =
            UserRelay::retrieve_from_store(&JsValue::from_str("wss://example.com"))
                .await
                .expect("Error retrieving from store");
        assert_eq!(retrieved.url, "wss://example.com");
        Ok(())
    }
}
