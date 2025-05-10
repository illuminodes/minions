use gloo::utils::format::JsValueSerdeExt;

// use crate::widgets::leaflet::LatLng;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GeolocationCoordinates {
    pub accuracy: f64,
    pub altitude: Option<f64>,
    #[serde(rename = "altitudeAccuracy")]
    pub altitude_accuracy: Option<f64>,
    pub latitude: f64,
    pub longitude: f64,
    pub speed: Option<f64>,
}
impl From<GeolocationCoordinates> for web_sys::wasm_bindgen::JsValue {
    fn from(val: GeolocationCoordinates) -> Self {
        Self::from_serde(&val).unwrap_or_default()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeolocationPosition {
    pub coords: GeolocationCoordinates,
    pub timestamp: f64,
}
impl GeolocationPosition {
    pub async fn locate() -> Result<Self, web_sys::wasm_bindgen::JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| web_sys::wasm_bindgen::JsValue::from_str("No window available"))?;
        let geolocation = window.navigator().geolocation()?;
        let (sender, receiver) = yew::platform::pinned::oneshot::channel::<Self>();
        let on_success: web_sys::js_sys::Function =
            web_sys::wasm_bindgen::closure::Closure::once_into_js(
                move |event: web_sys::Geolocation| {
                    if let Ok(geo) = Self::try_from(event) {
                        let _ = sender.send(geo);
                    }
                },
            )
            .into();
        geolocation.get_current_position(&on_success)?;
        receiver
            .await
            .map_err(|e| web_sys::wasm_bindgen::JsValue::from_str(&e.to_string()))
    }
}
impl TryFrom<web_sys::wasm_bindgen::JsValue> for GeolocationPosition {
    type Error = web_sys::wasm_bindgen::JsValue;
    fn try_from(value: web_sys::wasm_bindgen::JsValue) -> Result<Self, Self::Error> {
        let value = value
            .into_serde()
            .map_err(|e| web_sys::wasm_bindgen::JsValue::from_str(&e.to_string()))?;
        Ok(value)
    }
}
impl TryInto<web_sys::wasm_bindgen::JsValue> for GeolocationPosition {
    type Error = web_sys::wasm_bindgen::JsValue;
    fn try_into(self) -> Result<web_sys::wasm_bindgen::JsValue, Self::Error> {
        web_sys::wasm_bindgen::JsValue::from_serde(&self)
            .map_err(|e| web_sys::wasm_bindgen::JsValue::from_str(&e.to_string()))
    }
}
impl TryFrom<web_sys::Geolocation> for GeolocationPosition {
    type Error = web_sys::wasm_bindgen::JsValue;
    fn try_from(coords: web_sys::Geolocation) -> Result<Self, Self::Error> {
        let js_value: web_sys::wasm_bindgen::JsValue = coords.into();
        js_value
            .into_serde()
            .map_err(|e| web_sys::wasm_bindgen::JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn __get_geolocation() {
        let position = GeolocationPosition::locate().await;
        assert!(position.is_ok());
    }
}
