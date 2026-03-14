use web_sys::wasm_bindgen::{JsCast, JsValue};

fn subtle_crypto() -> Result<web_sys::SubtleCrypto, crate::MinionError> {
    let window = web_sys::window().ok_or_else(|| {
        crate::MinionError::CryptoError(JsValue::from_str("no global `window` exists"))
    })?;
    let crypto = window.crypto().map_err(crate::MinionError::CryptoError)?;
    Ok(crypto.subtle())
}

pub async fn import_key_array(
    p_key: web_sys::js_sys::Object,
) -> Result<web_sys::CryptoKey, crate::MinionError> {
    let subtle = subtle_crypto()?;
    let usage_tags: web_sys::js_sys::Array =
        [JsValue::from_str("encrypt"), JsValue::from_str("decrypt")]
            .iter()
            .collect();
    let key = subtle
        .import_key_with_object(
            "raw",
            &p_key,
            &web_sys::AesKeyGenParams::new("AES-GCM", 256).into(),
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
    js_value: web_sys::CryptoKey,
) -> Result<web_sys::js_sys::ArrayBuffer, crate::MinionError> {
    let subtle = subtle_crypto()?;
    let key = wasm_bindgen_futures::JsFuture::from(
        subtle
            .export_key("raw", &js_value)
            .map_err(crate::MinionError::CryptoError)?,
    )
    .await
    .map_err(crate::MinionError::CryptoError)?;
    Ok(key.into())
}
