use js_sys::{ArrayBuffer, Object, Uint8Array};
use nostro2::userkeys::UserKeys;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AesKeyGenParams, CryptoKey, SubtleCrypto};

fn crypto_subtle() -> Result<SubtleCrypto, JsValue> {
    let window = web_sys::window().ok_or(JsValue::from_str("No window available"))?;
    let crypto = window.crypto()?;
    Ok(crypto.subtle())
}
fn user_keys_to_object(user_keys: &UserKeys) -> Object {
    let secret_bytes = user_keys.get_secret_key();
    let array = Uint8Array::from(&secret_bytes[..]);
    array.buffer().into()
}

pub async fn user_keys_to_crypto(user_keys: &UserKeys) -> Result<CryptoKey, JsValue> {
    let key_object = user_keys_to_object(user_keys);
    let crypto = crypto_subtle()?;
    let algo = AesKeyGenParams::new("AES-GCM", 256);
    let usage_tags: js_sys::Array =
        vec![JsValue::from_str("encrypt"), JsValue::from_str("decrypt")]
            .iter()
            .collect();
    let key = crypto.import_key_with_object("raw", &key_object, &algo, true, &usage_tags)?;
    let key: JsValue = JsFuture::from(key).await?;
    Ok(key.dyn_into()?)
}

pub async fn crypto_to_user_keys(
    js_value: CryptoKey,
    extractable: bool,
) -> Result<UserKeys, JsValue> {
    let crypto = crypto_subtle()?;
    let key = JsFuture::from(crypto.export_key("raw", &js_value)?).await?;
    let key_array: ArrayBuffer = key.into();
    let key_array = Uint8Array::new(&key_array);
    let key_array = key_array.to_vec();
    let key_hex = key_array
        .iter()
        .map(|x| format!("{:02x}", x))
        .collect::<String>();
    match extractable {
        true => Ok(UserKeys::new_extractable(&key_hex).unwrap()),
        false => Ok(UserKeys::new(&key_hex).unwrap()),
    }
}
