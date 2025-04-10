use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::{AesKeyGenParams, CryptoKey, SubtleCrypto};

pub struct BrowserCrypto {
    crypto: SubtleCrypto,
}
impl Default for BrowserCrypto {
    fn default() -> Self {
        let window = web_sys::window().expect("no global `window` exists");
        let crypto = window.crypto().expect("no global `crypto` exists");
        Self {
            crypto: crypto.subtle(),
        }
    }
}
pub enum KeyGenParams {
    AesKeyGenParams,
}
impl From<KeyGenParams> for AesKeyGenParams {
    fn from(val: KeyGenParams) -> Self {
        match val {
            KeyGenParams::AesKeyGenParams => AesKeyGenParams::new("AES-GCM", 256),
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
    ) -> Result<CryptoKey, JsValue> {
        let usage_tags: web_sys::js_sys::Array =
            [JsValue::from_str("encrypt"), JsValue::from_str("decrypt")]
                .iter()
                .collect();
        let key = self.crypto.import_key_with_object(
            "raw",
            &p_key,
            &KeyGenParams::AesKeyGenParams.into(),
            true,
            &usage_tags,
        )?;
        let key: JsValue = wasm_bindgen_futures::JsFuture::from(key).await?;
        key.dyn_into()
    }
    pub async fn export_raw_key(
        &self,
        js_value: CryptoKey,
    ) -> Result<web_sys::js_sys::ArrayBuffer, JsValue> {
        let key =
            wasm_bindgen_futures::JsFuture::from(self.crypto.export_key("raw", &js_value)?).await?;
        Ok(key.into())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use wasm_bindgen_test::*;
//     wasm_bindgen_test_configure!(run_in_browser);
//     #[wasm_bindgen_test]
//     async fn _test_crypto_key_from_bytes() {
//         let crypto = BrowserCrypto::default();
//         let key = crypto.crypto_key_from_bytes(&[0; 32]).await.unwrap();
//         assert_eq!(key.type_(), "secret");
//     }
//     #[wasm_bindgen_test]
//     async fn _test_crypto_key_to_hex() {
//         let crypto = BrowserCrypto::default();
//         let key = crypto.crypto_key_from_bytes(&[0; 32]).await.unwrap();
//         let hex = crypto.crypto_key_to_hex(key).await.unwrap();
//         assert_eq!(hex.len(), 64);
//     }
// }
