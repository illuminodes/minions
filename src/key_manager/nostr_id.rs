use nostro2::userkeys::UserKeys;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::CryptoKey;

use crate::browser_api::{
    crypto::{crypto_to_user_keys, user_keys_to_crypto},
    indexed_db::IdbStoreManager,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserIdentity {
    id: String,
    crypto_key: CryptoKey,
}

impl UserIdentity {
    pub async fn find_local_identity() -> Result<Self, JsValue>
    where
        Self: IdbStoreManager,
    {
        let crypto_key = Self::retrieve::<CryptoKey>("privateKey")?
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(UserIdentity {
            id: "privateKey".to_string(),
            crypto_key,
        })
    }
    pub async fn new_user_identity() -> Result<Self, JsValue> {
        let user_key = UserKeys::generate_extractable();
        let crypto_key: CryptoKey = user_keys_to_crypto(&user_key).await?.into();
        if let Err(e) = Self::save_value_to_store(crypto_key.clone().into(), "privateKey")?.await {
            gloo::console::error!("Error saving key: ", format!("{:?}", e));
        }
        Ok(Self {
            id: "privateKey".to_string(),
            crypto_key,
        })
    }
    pub async fn from_new_keys(keys: UserKeys) -> Result<Self, JsValue> {
        let crypto_key: CryptoKey = user_keys_to_crypto(&keys).await?.into();
        if let Err(e) = Self::save_value_to_store(crypto_key.clone().into(), "privateKey")?.await {
            gloo::console::error!("Error saving key: ", format!("{:?}", e));
        }
        Ok(UserIdentity {
            id: "privateKey".to_string(),
            crypto_key,
        })
    }
    pub async fn get_user_keys(&self) -> Result<UserKeys, JsValue> {
        crypto_to_user_keys(self.crypto_key.clone(), true).await
    }
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
}

impl IdbStoreManager for UserIdentity {
    fn store_name() -> &'static str {
        "user_identity"
    }
    fn db_name() -> &'static str {
        "nostr"
    }
    fn db_version() -> u32 {
        1
    }
    fn document_key(&self) -> JsValue {
        JsValue::from_str(&self.id)
    }
    fn upgrade_db(event: web_sys::Event) -> Result<(), JsValue> {
        if event.target().is_none() {
            return Err(JsValue::from_str("Error upgrading database"));
        };
        let target = event.target().unwrap();
        let db = target
            .dyn_into::<web_sys::IdbOpenDbRequest>()?
            .result()?
            .dyn_into::<web_sys::IdbDatabase>()?;
        let _ = db.create_object_store("user_identity")?;
        Ok(())
    }
}
