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
    pub type Control;
    pub type TileLayer;

    #[wasm_bindgen(constructor, js_namespace = L, js_name = map)]
    pub fn map(id: &str) -> LeafletMap;
    #[wasm_bindgen(method, js_name = setView)]
    pub fn set_view(this: &LeafletMap, coords: &JsValue, zoom: u8);
    #[wasm_bindgen(method, structural, indexing_getter)]
    pub fn get(this: &LeafletMap, prop: &str) -> Control;
    #[wasm_bindgen(method)]
    pub fn on(this: &LeafletMap, event: &str, callback: Function);

    #[wasm_bindgen(method)]
    pub fn getZoom(this: &LeafletMap) -> f64;
    #[wasm_bindgen(method)]
    pub fn setZoom(this: &LeafletMap, zoom: f64);
    #[wasm_bindgen(method)]
    pub fn zoomIn(this: &LeafletMap);
    #[wasm_bindgen(method)]
    pub fn zoomOut(this: &LeafletMap);
    // Pane methods
    #[wasm_bindgen(method, js_name = "createPane")]
    pub fn create_pane(this: &LeafletMap, name: &str);
    #[wasm_bindgen(method, js_name = "getPane")]
    pub fn get_pane(this: &LeafletMap, name: &str) -> web_sys::Element;

    #[wasm_bindgen(method)]
    pub fn disable(this: &Control);
    #[wasm_bindgen(method)]
    pub fn addTo(this: &TileLayer, map: &LeafletMap);

    // Geolocation methods
    #[wasm_bindgen(method, js_name = "locate")]
    pub fn locate(this: &LeafletMap);

    #[wasm_bindgen(method, js_name = "locate")]
    pub fn locate_with_options(this: &LeafletMap, options: JsValue);

    #[wasm_bindgen(method, js_name = "stopLocate")]
    pub fn stop_locate(this: &LeafletMap);

    #[wasm_bindgen(method, js_name = "watchLocation")]
    pub fn watch_location(this: &LeafletMap);
}

// Add a struct for locate options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeafletLocateOptions {
    pub watch: bool,
    pub set_view: bool,
    pub max_zoom: f64,
    pub timeout: u32,
    pub maximum_age: u32,
    pub enable_high_accuracy: bool,
}

impl Default for LeafletLocateOptions {
    fn default() -> Self {
        Self {
            watch: false,
            set_view: true,
            max_zoom: 16.0,
            timeout: 10000,
            maximum_age: 0,
            enable_high_accuracy: false,
        }
    }
}

impl TryInto<JsValue> for LeafletLocateOptions {
    type Error = JsValue;
    fn try_into(self) -> Result<JsValue, Self::Error> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }
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

    pub fn zoom_level(&self) -> f64 {self.getZoom()}
    pub fn set_zoom_level(&self, zoom: f64) {self.setZoom(zoom)}
    pub fn zoom_in(&self) {self.zoomIn()}
    pub fn zoom_out(&self) {self.zoomOut()}

    pub fn create_map_pane(&self, name: &str) {self.create_pane(name)}
    pub fn get_map_pane(&self, name: &str) -> Option<web_sys::Element> {
        match self.get_pane(name) {
            pane if pane.is_undefined() => None,
            pane => Some(pane),
        }
    }
    pub fn start_locate(&self, options: Option<LeafletLocateOptions>) {
        match options {
            Some(opts) => {
                if let Ok(js_opts) = opts.try_into() {
                    self.locate_with_options(js_opts);
                }
            }
            None => self.locate(),
        }
    }

    pub fn stop_location_watch(&self) {self.stop_locate();}
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
