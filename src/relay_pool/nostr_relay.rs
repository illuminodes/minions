use wasm_bindgen::JsValue;

use crate::browser_api::indexed_db::IdbStoreManager;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UserRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
impl UserRelay {
    pub async fn get_local_relays() -> Result<Vec<Self>, JsValue>
    where
        Self: IdbStoreManager,
    {
        Self::retrieve_all_from_store::<Self>()?
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
impl TryFrom<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}
impl TryInto<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_into(self) -> Result<JsValue, Self::Error> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }
}
impl IdbStoreManager for UserRelay {
    fn store_name() -> &'static str {
        "user_relays"
    }
    fn db_name() -> &'static str {
        "nostr"
    }
    fn db_version() -> u32 {
        1
    }
    fn document_key(&self) -> JsValue {
        JsValue::from_str(&self.url)
    }
    fn upgrade_db(_event: web_sys::Event) -> Result<(), JsValue> {
        Ok(())
    }
}
