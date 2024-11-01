use super::component::LeafletComponent;
use crate::browser_api::GeolocationCoordinates;
use crate::relay_pool::NostrProps;
use crate::widgets::leaflet::IconOptions;
use crate::widgets::leaflet::{LatLng, LeafletLocateOptions, LeafletMap};
use js_sys;
use wasm_bindgen::JsValue;
use web_sys::MouseEvent;
use yew::prelude::*;

#[function_component(LeafletTest)]
pub fn leaflet_test() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let map = use_state(|| None::<LeafletMap>);
    let markers = use_state(|| Vec::<(f64, f64)>::new());
    let location_name = use_state(|| String::new());

    let send_test_event = {
        let note_sender = relay_ctx.send_note.clone();
        let markers = markers.clone();
        let map = map.clone();

        Callback::from(move |_| {
            // Array of test locations
            let test_locations = vec![
                (28.4089, 76.9699, "Gurugram", "red"),
                (28.6139, 77.2090, "Delhi", "green"),
                (28.7041, 77.1025, "New Delhi", "gold"),
                (28.4595, 77.0266, "Gurugram Downtown", "violet"),
            ];

            for (lat, lng, location_name, color) in test_locations {
                let coords = GeolocationCoordinates {
                    latitude: lat,
                    longitude: lng,
                    accuracy: 10.0,
                    altitude: None,
                    altitude_accuracy: None,
                    speed: None,
                };

                if let Some(map_instance) = &*map {
                    let icon_options = IconOptions {
                        icon_url: format!("https://raw.githubusercontent.com/pointhi/leaflet-color-markers/master/img/marker-icon-2x-{}.png", color),
                        icon_size: Some(vec![25, 41]),
                        icon_anchor: Some(vec![12, 41]),
                    };

                    // Only add marker once, with custom icon
                    if let Ok(_marker) = map_instance.add_marker_with_icon(&coords, icon_options) {
                        let mut new_markers = (*markers).clone();
                        new_markers.push((coords.latitude, coords.longitude));
                        markers.set(new_markers);

                        web_sys::console::log_1(
                            &format!("Added {} marker for {}", color, location_name).into(),
                        );
                    }

                    // Update view for the last location
                    let lat_lng = LatLng {
                        lat: coords.latitude,
                        lng: coords.longitude,
                    };

                    if let Ok(js_coords) = lat_lng.try_into() {
                        map_instance.set_view(&js_coords, 10);
                    }
                }

                // Send Nostr event
                let content = serde_json::to_string(&coords).unwrap();
                let new_keys = nostro2::userkeys::UserKeys::generate();
                let new_note =
                    nostro2::notes::Note::new(&new_keys.get_public_key(), 27235, &content);
                let signed_note = new_keys.sign_nostr_event(new_note);
                note_sender.emit(signed_note);
            }

            crate::widgets::toastify::ToastifyOptions::new_event_received(
                "Added multiple colored markers...",
            )
            .show();
        })
    };

    let zoom_in = {
        let map = map.clone();
        Callback::from(move |_| {
            if let Some(map_ref) = &*map {
                map_ref.zoom_in();
                crate::widgets::toastify::ToastifyOptions::new_event_received("Zoomed in").show();
            }
        })
    };

    let zoom_out = {
        let map = map.clone();
        Callback::from(move |_| {
            if let Some(map_ref) = &*map {
                map_ref.zoom_out();
                crate::widgets::toastify::ToastifyOptions::new_event_received("Zoomed out").show();
            }
        })
    };

    let start_locate = {
        let map = map.clone();
        let markers = markers.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(map_instance) = &*map {
                let options = LeafletLocateOptions {
                    watch: true,
                    set_view: true,
                    max_zoom: 16.0,
                    timeout: 10000,
                    maximum_age: 0,
                    enable_high_accuracy: true,
                };

                let markers = markers.clone();
                let map_for_closure = map_instance.clone();
                let map_for_locate = map_instance.clone();

                map_instance.add_closure("locationfound", move |event: JsValue| {
                    web_sys::console::log_1(&"Location event received".into());
                    web_sys::console::log_1(&event);

                    // Try to get latitude and longitude directly from the event
                    let latitude = js_sys::Reflect::get(&event, &JsValue::from_str("latitude"))
                        .and_then(|v| Ok(v.as_f64().unwrap_or(0.0)));
                    let longitude = js_sys::Reflect::get(&event, &JsValue::from_str("longitude"))
                        .and_then(|v| Ok(v.as_f64().unwrap_or(0.0)));

                    if let (Ok(lat), Ok(lng)) = (latitude, longitude) {
                        web_sys::console::log_1(
                            &format!("Got coordinates: Lat: {}, Lng: {}", lat, lng).into(),
                        );

                        let geo_coords = GeolocationCoordinates {
                            latitude: lat,
                            longitude: lng,
                            accuracy: 10.0,
                            altitude: None,
                            altitude_accuracy: None,
                            speed: None,
                        };

                        let lat_lng = LatLng {
                            lat: geo_coords.latitude,
                            lng: geo_coords.longitude,
                        };

                        if let Ok(js_coords) = lat_lng.try_into() {
                            map_for_closure.set_view(&js_coords, 13);

                            if let Ok(_) = map_for_closure.add_leaflet_marker(&geo_coords) {
                                let mut current_markers = (*markers).clone();
                                current_markers.push((geo_coords.latitude, geo_coords.longitude));
                                markers.set(current_markers);

                                web_sys::console::log_1(
                                    &"Successfully updated location and added marker".into(),
                                );
                            }
                        }
                    } else {
                        // Try alternate event format with latlng property
                        if let Ok(latlng) =
                            js_sys::Reflect::get(&event, &JsValue::from_str("latlng"))
                        {
                            let lat = js_sys::Reflect::get(&latlng, &JsValue::from_str("lat"))
                                .and_then(|v| Ok(v.as_f64().unwrap_or(0.0)));
                            let lng = js_sys::Reflect::get(&latlng, &JsValue::from_str("lng"))
                                .and_then(|v| Ok(v.as_f64().unwrap_or(0.0)));

                            if let (Ok(lat), Ok(lng)) = (lat, lng) {
                                web_sys::console::log_1(
                                    &format!(
                                        "Got coordinates from latlng: Lat: {}, Lng: {}",
                                        lat, lng
                                    )
                                    .into(),
                                );

                                let geo_coords = GeolocationCoordinates {
                                    latitude: lat,
                                    longitude: lng,
                                    accuracy: 10.0,
                                    altitude: None,
                                    altitude_accuracy: None,
                                    speed: None,
                                };

                                let lat_lng = LatLng {
                                    lat: geo_coords.latitude,
                                    lng: geo_coords.longitude,
                                };

                                if let Ok(js_coords) = lat_lng.try_into() {
                                    map_for_closure.set_view(&js_coords, 13);

                                    if let Ok(_) = map_for_closure.add_leaflet_marker(&geo_coords) {
                                        let mut current_markers = (*markers).clone();
                                        current_markers
                                            .push((geo_coords.latitude, geo_coords.longitude));
                                        markers.set(current_markers);

                                        web_sys::console::log_1(
                                            &"Successfully updated location and added marker"
                                                .into(),
                                        );
                                    }
                                }
                            } else {
                                web_sys::console::error_1(
                                    &"Could not extract lat/lng from latlng object".into(),
                                );
                            }
                        } else {
                            web_sys::console::error_1(
                                &"Could not extract coordinates from event in any format".into(),
                            );
                        }
                    }
                });

                map_for_locate.start_locate(Some(options));
                crate::widgets::toastify::ToastifyOptions::new_event_received(
                    "Started location tracking",
                )
                .show();
            }
        })
    };

    let stop_locate = {
        let map = map.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(map_ref) = &*map {
                map_ref.stop_location_watch();
                crate::widgets::toastify::ToastifyOptions::new_event_received(
                    "Stopped location tracking",
                )
                .show();
            }
        })
    };

    html! {
        <div class="flex flex-col gap-4 p-4">
            <h1 class="text-2xl font-bold">{"Leaflet Map Test"}</h1>
            <LeafletComponent
                map_id="leaflet-map"
                markers={(*markers).clone()}
                show_location_name=true
                on_map_created={Callback::from({
                    let map = map.clone();
                    move |map_instance: LeafletMap| map.set(Some(map_instance))
                })}
                on_location_name_changed={Callback::from({
                    let location_name = location_name.clone();
                    move |name: String| location_name.set(name)
                })}
            />
            <div class="flex gap-2">
                <button
                    onclick={send_test_event}
                    class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                >
                    {"Send Test Location"}
                </button>
                <button
                    onclick={zoom_in}
                    class="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600"
                >
                    {"Zoom In"}
                </button>
                <button
                    onclick={zoom_out}
                    class="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600"
                >
                    {"Zoom Out"}
                </button>
                <button
                    onclick={start_locate}
                    class="px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600"
                >
                    {"Start Location Tracking"}
                </button>
                <button
                    onclick={stop_locate}
                    class="px-4 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600"
                >
                    {"Stop Location Tracking"}
                </button>
            </div>
        </div>
    }
}
