use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::{AesKeyGenParams, CryptoKey, SubtleCrypto};

pub struct BrowserCrypto {
    crypto: SubtleCrypto,
}
impl BrowserCrypto {
    pub fn new() -> Result<Self, crate::MinionError> {
        let window = web_sys::window().ok_or_else(|| {
            crate::MinionError::CryptoError(JsValue::from_str("no global `window` exists"))
        })?;
        let crypto = window.crypto().map_err(crate::MinionError::CryptoError)?;
        Ok(Self {
            crypto: crypto.subtle(),
        })
    }
}
pub enum KeyGenParams {
    AesKeyGenParams,
}
impl From<KeyGenParams> for AesKeyGenParams {
    fn from(val: KeyGenParams) -> Self {
        match val {
            KeyGenParams::AesKeyGenParams => Self::new("AES-GCM", 256),
        }
    }
}
impl From<KeyGenParams> for web_sys::js_sys::Object {
    fn from(val: KeyGenParams) -> Self {
        let key_params: web_sys::AesKeyGenParams = val.into();
        key_params.into()
    }
}
impl BrowserCrypto {
    pub async fn import_key_array(
        &self,
        p_key: web_sys::js_sys::Object,
    ) -> Result<CryptoKey, crate::MinionError> {
        let usage_tags: web_sys::js_sys::Array =
            [JsValue::from_str("encrypt"), JsValue::from_str("decrypt")]
                .iter()
                .collect();
        let key = self
            .crypto
            .import_key_with_object(
                "raw",
                &p_key,
                &KeyGenParams::AesKeyGenParams.into(),
                true,
                &usage_tags,
            )
            .map_err(crate::MinionError::CryptoError)?;
        let key: JsValue = wasm_bindgen_futures::JsFuture::from(key)
            .await
            .map_err(crate::MinionError::CryptoError)?;
        key.dyn_into().map_err(crate::MinionError::CryptoError)
    }
    pub async fn export_raw_key(
        &self,
        js_value: CryptoKey,
    ) -> Result<web_sys::js_sys::ArrayBuffer, crate::MinionError> {
        let key = wasm_bindgen_futures::JsFuture::from(
            self.crypto
                .export_key("raw", &js_value)
                .map_err(crate::MinionError::CryptoError)?,
        )
        .await
        .map_err(crate::MinionError::CryptoError)?;
        Ok(key.into())
    }
}
