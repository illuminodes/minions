use wasm_bindgen::JsValue;

use crate::browser_api::IdbStoreManager;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UserRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
impl TryFrom<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}
impl Into<JsValue> for UserRelay {
    fn into(self) -> JsValue {
        serde_wasm_bindgen::to_value(&self).unwrap()
    }
}
impl IdbStoreManager for UserRelay {
    fn config() -> crate::browser_api::IdbStoreConfig {
        crate::browser_api::IdbStoreConfig {
            db_version: 1,
            db_name: "test_db_relays",
            store_name: "user_relays",
            document_key: "url",
        }
    }
    fn key(&self) -> JsValue {
        JsValue::from_str(&self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn _relay_idb_manager() -> Result<(), JsValue> {
        let user_relay = UserRelay {
            url: "wss://example.com".to_string(),
            read: true,
            write: false,
        };
        user_relay
            .save_to_store()
            .await
            .expect("Error saving to store");
        let retrieved: UserRelay =
            UserRelay::retrieve_from_store(&JsValue::from_str("wss://example.com"))
                .await
                .expect("Error retrieving from store");
        assert_eq!(retrieved.url, "wss://example.com");
        Ok(())
    }
}
