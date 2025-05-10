use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::browser_api::GeolocationCoordinates;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NominatimLookup {
    place_id: i64,
    lat: String,
    lon: String,
    name: String,
    display_name: String,
}
impl NominatimLookup {
    #[must_use]
    pub fn long_as_f64(&self) -> f64 {
        self.lon.parse().unwrap_or(0.0)
    }
    #[must_use]
    pub fn lat_as_f64(&self) -> f64 {
        self.lat.parse().unwrap_or(0.0)
    }
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn id_str(&self) -> String {
        self.place_id.to_string()
    }
    pub async fn address(query: &str) -> Result<Vec<Self>, JsValue> {
        let url = format!("https://nominatim.openstreetmap.org/search?q={query}&format=jsonv2");
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window available."))?;
        let promise = window.fetch_with_str(&url);
        let response: Response = JsFuture::from(promise).await?.into();
        let response_text = response.text()?;
        if let Some(response_body) = JsFuture::from(response_text).await?.as_string() {
            let nominatim: Vec<Self> = serde_json::from_str(&response_body)
                .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
            Ok(nominatim)
        } else {
            Err(JsValue::from_str("No response body"))
        }
    }
    pub async fn reverse(coordinate: GeolocationCoordinates) -> Result<Self, JsValue> {
        let lat = coordinate.latitude;
        let lon = coordinate.longitude;
        let url = format!(
            "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={lat}&lon={lon}"
        );
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window available."))?;
        let promise = window.fetch_with_str(&url);
        let response: Response = JsFuture::from(promise).await?.into();
        let response_text = response.text()?;
        if let Some(response_body) = JsFuture::from(response_text).await?.as_string() {
            let nominatim: Self = serde_json::from_str(&response_body)
                .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
            Ok(nominatim)
        } else {
            Err(JsValue::from_str("No response body"))
        }
    }
}
impl Default for NominatimLookup {
    fn default() -> Self {
        Self {
            place_id: 0,
            lat: "0".to_string(),
            lon: "0".to_string(),
            name: String::new(),
            display_name: String::new(),
        }
    }
}
