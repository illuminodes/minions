use nostro2_signer::keypair::NostrKeypair;
use nostro2_signer::nostro2::NostrNote;
use nostro2_signer::nostro2::NostrSigner;
use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::CryptoKey;

use crate::browser_api::BrowserCrypto;
use crate::{
    browser_api::{IdbStoreConfig, IdbStoreManager},
    DB_NAME, DB_VERSION, IDENTITY_KEY, IDENTITY_STORE,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NostrIdType {
    Local(CryptoKey),
    Extension,
    Bunker(String),
}
impl From<NostrIdType> for JsValue {
    fn from(val: NostrIdType) -> Self {
        match val {
            NostrIdType::Local(key) => key.into(),
            NostrIdType::Extension => Self::from_str("Extension"),
            NostrIdType::Bunker(url) => Self::from_str(&url),
        }
    }
}
impl AsRef<JsValue> for NostrIdType {
    fn as_ref(&self) -> &JsValue {
        self.unchecked_ref()
    }
}
impl TryFrom<JsValue> for NostrIdType {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        if let Some(key) = value.dyn_ref::<CryptoKey>() {
            Ok(Self::Local(key.clone()))
        } else if let Some(url) = value.as_string() {
            if url.contains("Extension") {
                return Ok(Self::Extension);
            }
            Ok(Self::Bunker(url))
        } else {
            Err(JsValue::from_str("Not a valid NostrIdType"))
        }
    }
}
impl wasm_bindgen::JsCast for NostrIdType {
    fn instanceof(val: &JsValue) -> bool {
        val.is_instance_of::<CryptoKey>()
            || val.is_string()
            || val.as_string().is_some_and(|s| s == "Extension")
    }
    fn unchecked_from_js(val: JsValue) -> Self {
        if let Some(key) = val.dyn_ref::<CryptoKey>() {
            return Self::Local(key.clone());
        }
        if let Some(url) = val.as_string() {
            if url.contains("Extension") {
                return Self::Extension;
            }
            return Self::Bunker(url);
        }
        panic!("Not a valid NostrIdType");
    }
    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        val.unchecked_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserIdentity {
    pubkey: String,
    default: bool,
    tag: String,
    pub signer: NostrIdType,
}

impl UserIdentity {
    pub async fn find_identity() -> Result<Self, JsValue>
    where
        Self: IdbStoreManager,
    {
        Self::retrieve_from_store::<Self>(&JsValue::from_str("privateKey")).await
    }
    pub async fn get_pubkey(&self) -> Option<String> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await.ok()?;
                let slice = &web_sys::js_sys::Uint8Array::new(&keypair);
                let pubkey = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .ok()?
                    .public_key();
                Some(pubkey)
            }
            NostrIdType::Extension => None,
            NostrIdType::Bunker(_url) => None,
        }
    }
    pub async fn new_local_identity() -> Result<Self, JsValue> {
        let user_key = NostrKeypair::generate(true);
        Self::from_new_keys(user_key.clone()).await
    }
    pub async fn from_new_keys(keys: NostrKeypair) -> Result<Self, JsValue> {
        let array = keys.secret_key();
        let js_array = web_sys::js_sys::Uint8Array::from(array.as_slice());
        let crypto_key: CryptoKey = BrowserCrypto::default()
            .import_key_array(js_array.into())
            .await?;
        let user_identity = Self {
            pubkey: "privateKey".to_string(),
            default: true,
            tag: String::new(),
            signer: NostrIdType::Local(crypto_key),
        };
        user_identity.clone().save_to_store().await?;
        Ok(user_identity)
    }
    pub async fn sign_nostr_note(&self, note: &mut NostrNote) -> Result<(), JsValue> {
        match self.signer {
            NostrIdType::Local(ref key) => {
                let slice = BrowserCrypto::default().export_raw_key(key.clone()).await?;
                let vec_bytes = web_sys::js_sys::Uint8Array::new(&slice);
                let nostr_key: NostrKeypair = NostrKeypair::try_from(vec_bytes.to_vec().as_slice())
                    .map_err(|e| {
                        JsValue::from_str(&format!("Failed to convert to NostrKeypair: {e}"))
                    })?;
                nostr_key
                    .sign_nostr_note(note)
                    .map_err(|e| JsValue::from_str(&format!("Failed to sign note: {e}")))
            }
            NostrIdType::Extension => Err(JsValue::from_str("Refactoring Support")),
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn decrypt_nip04(&self, note: NostrNote) -> Result<String, JsValue> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await?;
                let slice = web_sys::js_sys::Uint8Array::new(&keypair);
                let keypair = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(keypair
                    .decrypt_note(
                        &note,
                        &note.pubkey,
                        &nostro2_signer::keypair::EncryptionScheme::Nip04,
                    )
                    .map_err(|e| JsValue::from_str(&e.to_string()))?
                    .to_string())
            }
            NostrIdType::Extension => Err(JsValue::from_str("Deprected")),
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn decrypt_nip44(&self, note: &NostrNote) -> Result<String, JsValue> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await?;
                let slice = web_sys::js_sys::Uint8Array::new(&keypair);
                let keypair = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(keypair
                    .decrypt_note(
                        note,
                        &note.pubkey,
                        &nostro2_signer::keypair::EncryptionScheme::Nip44,
                    )
                    .map_err(|e| JsValue::from_str(&e.to_string()))?
                    .to_string())
            }
            NostrIdType::Extension => Err(JsValue::from_str("Refactoring Support")),
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn sign_nip04(&self, note: &mut NostrNote, pubkey: String) -> Result<(), JsValue> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await?;
                let slice = web_sys::js_sys::Uint8Array::new(&keypair);
                let keypair = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                keypair
                    .sign_encrypted_note(
                        note,
                        &pubkey,
                        &nostro2_signer::keypair::EncryptionScheme::Nip04,
                    )
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(())
            }
            NostrIdType::Extension => Err(JsValue::from_str("Deprected")),
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn sign_nip44(&self, note: &mut NostrNote, pubkey: String) -> Result<(), JsValue> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await?;
                let slice = web_sys::js_sys::Uint8Array::new(&keypair);
                let keypair = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                keypair
                    .sign_encrypted_note(
                        note,
                        &pubkey,
                        &nostro2_signer::keypair::EncryptionScheme::Nip44,
                    )
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(())
            }
            NostrIdType::Extension => Err(JsValue::from_str("Refactoring Support")),
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn get_user_keys(&self) -> Result<NostrKeypair, JsValue> {
        if let NostrIdType::Local(signer) = &self.signer {
            let key_bytes = BrowserCrypto::default()
                .export_raw_key(signer.clone())
                .await?;
            let slice = web_sys::js_sys::Uint8Array::new(&key_bytes);
            Ok(
                NostrKeypair::try_from(slice.to_vec().as_slice()).map_err(|e| {
                    JsValue::from_str(&format!("Failed to convert to NostrKeypair: {e}"))
                })?,
            )
        } else {
            Err(JsValue::from_str("No local key"))
        }
    }
    pub async fn create_giftwrap(
        &self,
        inner_note: NostrNote,
        kind: u32,
    ) -> Result<NostrNote, JsValue> {
        // Serialize the inner note to string without signing
        let inner_content = serde_json::to_string(&inner_note)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize inner note: {e}")))?;

        // Create the outer wrapper note (unsigned)
        let giftwrap = NostrNote {
            content: inner_content,
            pubkey: self
                .get_pubkey()
                .await
                .ok_or_else(|| JsValue::from_str("Failed to get pubkey"))?,
            kind,
            ..Default::default()
        };

        // Return the unsigned giftwrap
        Ok(giftwrap)
    }

    pub async fn unwrap_giftwrap(&self, giftwrap: &NostrNote) -> Result<NostrNote, JsValue> {
        let decrypted_content = self.decrypt_nip44(giftwrap).await?;
        serde_json::from_str::<NostrNote>(&decrypted_content)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse unwrapped note: {e}")))
    }
}
impl From<UserIdentity> for JsValue {
    fn from(val: UserIdentity) -> Self {
        let obj = web_sys::js_sys::Object::new();
        let _ = web_sys::js_sys::Reflect::set(
            &obj,
            &Self::from_str("pubkey"),
            &Self::from_str(&val.pubkey),
        );
        let _ =
            web_sys::js_sys::Reflect::set(&obj, &Self::from_str("crypto_key"), &val.signer.into());
        obj.into()
    }
}
impl TryFrom<JsValue> for UserIdentity {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        let obj = web_sys::js_sys::Object::try_from(&value)
            .ok_or_else(|| JsValue::from_str("Not an object"))?;
        let pubkey = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("pubkey"))?
            .as_string()
            .ok_or_else(|| JsValue::from_str("id not found"))?;
        let crypto_key = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("crypto_key"))?;
        let signer = crypto_key.dyn_into::<NostrIdType>()?;
        let default = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("default"))?
            .as_bool()
            .unwrap_or(false);
        let tag = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("tag"))?
            .as_string()
            .unwrap_or_else(String::new);
        Ok(Self {
            pubkey,
            default,
            tag,
            signer,
        })
    }
}
impl IdbStoreManager for UserIdentity {
    fn config() -> IdbStoreConfig {
        IdbStoreConfig {
            store_name: IDENTITY_STORE,
            db_name: DB_NAME,
            db_version: DB_VERSION,
            document_key: IDENTITY_KEY,
        }
    }
    fn key(&self) -> JsValue {
        JsValue::from_str(&self.pubkey)
    }
}
