use web_sys::wasm_bindgen::JsValue;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UserRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
impl TryFrom<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
impl From<UserRelay> for JsValue {
    fn from(val: UserRelay) -> Self {
        serde_wasm_bindgen::to_value(&val).unwrap_or_default()
    }
}
