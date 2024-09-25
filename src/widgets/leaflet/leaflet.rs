use js_sys::{Function, Object};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{convert::FromWasmAbi, prelude::*};

use crate::browser_api::geolocation::GeolocationCoordinates;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}
impl TryInto<JsValue> for LatLng {
    type Error = JsValue;
    fn try_into(self) -> Result<JsValue, Self::Error> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }
}
impl TryFrom<JsValue> for LatLng {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}
impl From<&GeolocationCoordinates> for LatLng {
    fn from(value: &GeolocationCoordinates) -> Self {
        Self { lat: value.latitude, lng: value.longitude }
    }
}
impl From<GeolocationCoordinates> for LatLng {
    fn from(value: GeolocationCoordinates) -> Self {
        Self { lat: value.latitude, lng: value.longitude }
    }
}

#[wasm_bindgen]
extern "C" {
    pub type L;
    #[wasm_bindgen(static_method_of = L)]
    pub fn map(id: &str) -> LeafletMap;
    #[wasm_bindgen(static_method_of = L, js_name = tileLayer)]
    pub fn tile_layer(url: &str, options: JsValue) -> TileLayer;
    #[wasm_bindgen(static_method_of = L, js_name = marker)]
    pub fn marker(coords: &JsValue, options: JsValue) -> NewMarker;
}
impl L {
    pub fn render_map(id: &str, coords: &GeolocationCoordinates) -> Result<LeafletMap, JsValue> {
        let lat_lng: LatLng = coords.into();
        let new_coords: JsValue = lat_lng.try_into()?;
        let map = L::map(id);
        map.get("doubleClickZoom").disable();
        map.set_view(&new_coords, 13);
        let map_options: JsValue = Object::new().into();
        L::tile_layer(
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            map_options,
        )
        .addTo(&map);
        Ok(map)
    }
}
#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone, PartialEq)]
    pub type LeafletMap;
    #[wasm_bindgen(constructor, js_namespace = L, js_name = map)]
    pub fn map(id: &str) -> LeafletMap;
    #[wasm_bindgen(method, js_name = setView)]
    pub fn set_view(this: &LeafletMap, coords: &JsValue, zoom: u8);
    #[wasm_bindgen(method, structural, indexing_getter)]
    pub fn get(this: &LeafletMap, prop: &str) -> Control;
    #[wasm_bindgen(method)]
    pub fn on(this: &LeafletMap, event: &str, callback: Function);

    pub type Control;
    #[wasm_bindgen(method)]
    pub fn disable(this: &Control);

    pub type TileLayer;
    #[wasm_bindgen(method)]
    pub fn addTo(this: &TileLayer, map: &LeafletMap);
}
impl LeafletMap {
    pub fn add_leaflet_marker(&self, coords: &GeolocationCoordinates) -> Result<Marker, JsValue> {
        let lat_lng: LatLng = coords.into();
        let new_coords: JsValue = lat_lng.try_into()?;
        let marker_options = LeafletMarkerOptions::default();
        let marker = L::marker(&new_coords, marker_options.try_into()?).addTo(self);
        Ok(marker)
    }
    pub fn add_closure<T, A>(&self, event: &str, callback: T)
    where
        T: FnMut(A) + 'static,
        A: FromWasmAbi + 'static,
    {
        let map_closure = Closure::<dyn FnMut(A)>::new(callback);
        let map_function: Function = map_closure.into_js_value().into();
        self.on(event, map_function);
    }
}
#[wasm_bindgen]
extern "C" {
    pub type NewMarker;
    #[wasm_bindgen(method)]
    pub fn addTo(this: &NewMarker, map: &LeafletMap) -> Marker;

    #[derive(Debug, Clone, PartialEq)]
    pub type Marker;
    #[wasm_bindgen(method)]
    pub fn on(this: &Marker, event: &str, callback: Function);
    #[wasm_bindgen(method, js_name = setLatLng)]
    pub fn set_lat_lng(this: &Marker, coords: &JsValue) -> Marker;
    #[wasm_bindgen(method)]
    pub fn remove(this: &Marker);
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeafletMarkerOptions {
    draggable: bool,
    #[serde(rename = "autoPan")]
    auto_pan: bool,
}
impl Default for LeafletMarkerOptions {
    fn default() -> Self {
        Self {
            draggable: false,
            auto_pan: true,
        }
    }
}
impl TryInto<JsValue> for LeafletMarkerOptions {
    type Error = JsValue;
    fn try_into(self) -> Result<JsValue, Self::Error> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }
}
impl TryFrom<JsValue> for LeafletMarkerOptions {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}
