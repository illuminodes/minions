use nostro2_signer::nostro2::keypair::NostrKeypair;
use nostro2_signer::nostro2::notes::NostrNote;
use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::CryptoKey;

use crate::{
    browser_api::{BrowserCrypto, IdbStoreConfig, IdbStoreManager},
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
            NostrIdType::Extension => JsValue::from_str("Extension"),
            NostrIdType::Bunker(url) => JsValue::from_str(&url),
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
            Ok(NostrIdType::Local(key.clone()))
        } else if let Some(url) = value.as_string() {
            if url.contains("Extension") {
                return Ok(NostrIdType::Extension);
            }
            Ok(NostrIdType::Bunker(url))
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
            return NostrIdType::Local(key.clone());
        }
        if let Some(url) = val.as_string() {
            if url.contains("Extension") {
                return NostrIdType::Extension;
            }
            return NostrIdType::Bunker(url);
        }
        panic!("Not a valid NostrIdType");
    }
    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        val.unchecked_ref()
    }
}
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub type NostrSignerExtension;

    #[wasm_bindgen(method, catch, js_name = getPublicKey)]
    pub async fn get_public_key(this: &NostrSignerExtension) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch, js_name = signEvent)]
    pub async fn sign_event(
        this: &NostrSignerExtension,
        event: JsValue,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub async fn get_relays(this: &NostrSignerExtension) -> Result<JsValue, JsValue>;

    pub type Nip04Crypto;

    #[wasm_bindgen(method, catch)]
    pub async fn encrypt(
        this: &Nip04Crypto,
        pubkey: JsValue,
        data: JsValue,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub async fn decrypt(
        this: &Nip04Crypto,
        pubkey: JsValue,
        ciphertext: JsValue,
    ) -> Result<JsValue, JsValue>;

    pub type Nip44Crypto;

    #[wasm_bindgen(method, catch)]
    pub async fn encrypt(
        this: &Nip44Crypto,
        pubkey: JsValue,
        data: JsValue,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub async fn decrypt(
        this: &Nip44Crypto,
        pubkey: JsValue,
        ciphertext: JsValue,
    ) -> Result<JsValue, JsValue>;
}
impl NostrSignerExtension {
    pub async fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or(JsValue::from_str("No window"))?;
        let window_nostr = window.get("nostr").ok_or(JsValue::from_str("No nostr"))?;
        Ok(window_nostr.unchecked_into())
    }
    pub async fn nip04(&self) -> Result<Nip04Crypto, JsValue> {
        Ok(
            web_sys::js_sys::Reflect::get(self, &JsValue::from_str("nip04"))?
                .unchecked_into::<Nip04Crypto>(),
        )
    }
    pub async fn nip44(&self) -> Result<Nip44Crypto, JsValue> {
        Ok(
            web_sys::js_sys::Reflect::get(self, &JsValue::from_str("nip44"))?
                .unchecked_into::<Nip44Crypto>(),
        )
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
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await.ok()?;
                let pubkey = signer.get_public_key().await.ok()?.as_string()?;
                Some(pubkey)
            }
            NostrIdType::Bunker(url) => {
                gloo::console::log!("Bunker url: {:?}", url);
                None
            }
        }
    }
    pub async fn new_local_identity() -> Result<Self, JsValue> {
        let user_key = NostrKeypair::generate(true);
        Self::from_new_keys(user_key.clone()).await
    }
    pub async fn new_extension_identity() -> Result<Self, JsValue> {
        NostrSignerExtension::new().await?.get_public_key().await?;
        let new_identity = UserIdentity {
            pubkey: "privateKey".to_string(),
            default: true,
            tag: String::new(),
            signer: NostrIdType::Extension,
        };
        Ok(new_identity)
    }
    pub async fn from_new_keys(keys: NostrKeypair) -> Result<Self, JsValue> {
        let array = keys.get_secret_key();
        let js_array = web_sys::js_sys::Uint8Array::from(array.as_slice());
        let crypto_key: CryptoKey = BrowserCrypto::default()
            .import_key_array(js_array.into())
            .await?;
        let user_identity = UserIdentity {
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
                let nostr_key: NostrKeypair =
                    NostrKeypair::try_from(vec_bytes.to_vec().as_slice()).unwrap();
                nostr_key.sign_nostr_event(note);
                Ok(())
            }
            NostrIdType::Extension => {
                #[cfg(target_arch = "wasm32")]
                {
                    let nostr_signer = NostrSignerExtension::new().await?;
                    let signed_note_js = nostr_signer.sign_event(note.clone().into()).await?;
                    let signed_note: NostrNote = signed_note_js.try_into()?;
                    *note = signed_note;
                }
                Ok(())
            }
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
                keypair
                    .decrypt_nip_04_content(&note)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await?;
                let new_note = signer
                    .nip04()
                    .await?
                    .decrypt(note.pubkey.clone().into(), note.content.clone().into())
                    .await?;
                Ok(new_note.try_into()?)
            }

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
                keypair
                    .decrypt_nip_44_content(note)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await?;
                let new_note = signer
                    .nip44()
                    .await?
                    .decrypt(note.pubkey.clone().into(), note.content.clone().into())
                    .await?;
                Ok(new_note.try_into()?)
            }
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn encrypt_nip44(
        &self,
        cleartext: String,
        pubkey: String,
    ) -> Result<String, JsValue> {
        match &self.signer {
            NostrIdType::Local(key) => {
                let key = key.clone();
                let keypair = BrowserCrypto::default().export_raw_key(key).await?;
                let slice = web_sys::js_sys::Uint8Array::new(&keypair);
                let keypair = NostrKeypair::try_from(slice.to_vec().as_slice())
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                let encrypted = keypair
                    .encrypt_nip_44_plaintext(&cleartext, pubkey)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(encrypted)
            }
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await?;
                let encrypted = signer
                    .nip44()
                    .await?
                    .encrypt(pubkey.into(), cleartext.into())
                    .await?;
                Ok(encrypted.try_into()?)
            }
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
                    .sign_nip_04_encrypted(note, pubkey)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(())
            }
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await?;
                let encrypted_note_js = signer
                    .nip04()
                    .await?
                    .encrypt(pubkey.into(), note.content.clone().into())
                    .await?;

                note.content = encrypted_note_js.try_into()?;

                // Sign the note with updated content
                #[cfg(target_arch = "wasm32")]
                {
                    let signed_note_js = signer.sign_event(note.clone().into()).await?;
                    let signed_note: NostrNote = signed_note_js.try_into()?;
                    *note = signed_note;
                }
                Ok(())
            }
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
                    .sign_nip_44_encrypted(note, pubkey)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(())
            }
            NostrIdType::Extension => {
                let signer = NostrSignerExtension::new().await?;
                let new_content = signer
                    .nip44()
                    .await?
                    .encrypt(pubkey.into(), note.content.clone().into())
                    .await?;
                note.content = new_content.try_into()?;

                // Sign the note with updated content
                #[cfg(target_arch = "wasm32")]
                {
                    let signed_note_js = signer.sign_event(note.clone().into()).await?;
                    let signed_note: NostrNote = signed_note_js.try_into()?;
                    *note = signed_note;
                }
                Ok(())
            }
            NostrIdType::Bunker(_) => Err(JsValue::from_str("No Bunker support yet")),
        }
    }
    pub async fn get_user_keys(&self) -> Result<NostrKeypair, JsValue> {
        if let NostrIdType::Local(signer) = &self.signer {
            let key_bytes = BrowserCrypto::default()
                .export_raw_key(signer.clone())
                .await?;
            let slice = web_sys::js_sys::Uint8Array::new(&key_bytes);
            Ok(NostrKeypair::try_from(slice.to_vec().as_slice()).unwrap())
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
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize inner note: {}", e)))?;

        // Create the outer wrapper note (unsigned)
        let giftwrap = NostrNote {
            content: inner_content,
            pubkey: self
                .get_pubkey()
                .await
                .ok_or(JsValue::from_str("Failed to get pubkey"))?,
            kind,
            ..Default::default()
        };

        // Return the unsigned giftwrap
        Ok(giftwrap)
    }

    pub async fn unwrap_giftwrap(&self, giftwrap: &NostrNote) -> Result<NostrNote, JsValue> {
        let decrypted_content = self.decrypt_nip44(giftwrap).await?;
        serde_json::from_str::<NostrNote>(&decrypted_content)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse unwrapped note: {}", e)))
    }
}
impl From<UserIdentity> for JsValue {
    fn from(val: UserIdentity) -> Self {
        let obj = web_sys::js_sys::Object::new();
        web_sys::js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("pubkey"),
            &JsValue::from_str(&val.pubkey),
        )
        .unwrap();
        web_sys::js_sys::Reflect::set(&obj, &JsValue::from_str("crypto_key"), &val.signer.into())
            .unwrap();
        obj.into()
    }
}
impl TryFrom<JsValue> for UserIdentity {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        let obj =
            web_sys::js_sys::Object::try_from(&value).ok_or(JsValue::from_str("Not an object"))?;
        let pubkey = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("pubkey"))?
            .as_string()
            .ok_or(JsValue::from_str("id not found"))?;
        let crypto_key = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("crypto_key"))?;
        let signer = crypto_key.dyn_into::<NostrIdType>()?;
        let default = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("default"))?
            .as_bool()
            .unwrap_or(false);
        let tag = web_sys::js_sys::Reflect::get(obj, &JsValue::from_str("tag"))?
            .as_string()
            .unwrap_or_else(|| "".to_string());
        Ok(UserIdentity {
            pubkey,
            signer,
            default,
            tag,
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
