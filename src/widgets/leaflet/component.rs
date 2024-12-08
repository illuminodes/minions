use super::leaflet::{LeafletMap, Marker, L};
use super::nominatim::NominatimLookup;
use super::{IconOptions, LeafletMapOptions};
use crate::browser_api::{GeolocationCoordinates, GeolocationPosition};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

// Make Props cloneable
#[derive(Properties, PartialEq, Clone)]
pub struct Props {
    pub map_id: AttrValue,
    #[prop_or_default]
    pub map_options: Option<LeafletMapOptions>,
    #[prop_or_default]
    pub location_icon_options: Option<IconOptions>,
    #[prop_or_default]
    pub on_map_created: Callback<LeafletMap>,
    #[prop_or_default]
    pub on_marker_created: Callback<Marker>,
    #[prop_or_default]
    pub on_location_changed: Callback<GeolocationCoordinates>,
    #[prop_or_default]
    pub on_location_name_changed: Callback<NominatimLookup>,
    #[prop_or_default]
    pub markers: Vec<(f64, f64)>, // Vector of (latitude, longitude) pairs
    #[prop_or_default]
    pub initial_zoom: Option<u8>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub style: Option<AttrValue>,
}

#[function_component(LeafletComponent)]
pub fn leaflet_component(props: &Props) -> Html {
    let map = use_state(|| None::<LeafletMap>);
    let markers = use_state(|| Vec::<Marker>::new());

    // Initial map setup
    {
        let map = map.clone();
        let markers = markers.clone();
        let map_id = props.map_id.clone();
        let on_map_created = props.on_map_created.clone();
        let on_location_changed = props.on_location_changed.clone();
        let on_marker_created = props.on_marker_created.clone();
        let on_location_name_changed = props.on_location_name_changed.clone();
        let initial_markers = props.markers.clone();
        let icon_options = props.location_icon_options.clone();
        let map_options = props.map_options.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(position) = GeolocationPosition::locate().await {
                    let coords = position.coords;
                    on_location_changed.emit(coords.clone());

                    if let Ok(map_instance) = L::render_map(&map_id, &coords, map_options) {
                        // Set map state and emit to parent
                        map.set(Some(map_instance.clone()));
                        on_map_created.emit(map_instance.clone());

                        match icon_options {
                            Some(options) => {
                                if let Ok(marker) =
                                    map_instance.add_marker_with_icon(&coords, options)
                                {
                                    let mut current_markers = (*markers).clone();
                                    current_markers.push(marker.clone());
                                    markers.set(current_markers);
                                    on_marker_created.emit(marker);
                                }
                            }
                            None => {
                                // Add marker for current location
                                if let Ok(marker) = map_instance.add_leaflet_marker(&coords) {
                                    let mut current_markers = (*markers).clone();
                                    current_markers.push(marker.clone());
                                    markers.set(current_markers);
                                    on_marker_created.emit(marker);
                                }
                            }
                        }

                        // Get location name
                        if let Ok(location) = NominatimLookup::reverse(coords).await {
                            on_location_name_changed.emit(location);
                        }

                        // Add markers from props
                        for (lat, lng) in initial_markers {
                            let coords = GeolocationCoordinates {
                                latitude: lat,
                                longitude: lng,
                                accuracy: 1.0,
                                altitude: None,
                                altitude_accuracy: None,
                                speed: None,
                            };
                            if let Ok(marker) = map_instance.add_leaflet_marker(&coords) {
                                let mut current_markers = (*markers).clone();
                                current_markers.push(marker.clone());
                                markers.set(current_markers);
                                on_marker_created.emit(marker);
                            }
                        }
                    }
                }
            });
            || ()
        });
    }

    html! {
        <div class={props.class.clone()}>
            <div
                id={props.map_id.clone()}
                style={props.style.clone().unwrap_or(AttrValue::from(""))}
                class="w-full h-full"
            />
        </div>
    }
}
