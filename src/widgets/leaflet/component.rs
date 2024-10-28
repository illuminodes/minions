use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use nostro2::relays::NostrFilter;
use crate::browser_api::geolocation::{GeolocationPosition, GeolocationCoordinates};
use crate::relay_pool::relay_pool::NostrProps;
use super::leaflet::{L, LeafletMap, Marker};
use super::nominatim::NominatimLookup;

#[function_component(LeafletComponent)]
pub fn leaflet_component() -> Html {
    let map_id = "leaflet-map";
    let map = use_state(|| None::<LeafletMap>);
    let marker = use_state(|| None::<Marker>);
    let location_name = use_state(|| String::new());
    
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let subscription_id = use_state(|| None);

    // Subscribe to geolocation events
    {
        let subscriber = relay_ctx.subscribe.clone();
        let id_handle = subscription_id.clone();
        
        use_effect_with((), move |_| {
            let geo_filter = NostrFilter::default()
                .new_kind(27235)
                .subscribe();
            
            id_handle.set(Some(geo_filter.id()));
            subscriber.emit(geo_filter);
            
            || {}
        });
    }

    // Initial map setup
    {
        let map = map.clone();
        let marker = marker.clone();
        let location_name = location_name.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                web_sys::console::log_1(&"Starting location fetch...".into());
                if let Ok(position) = GeolocationPosition::locate().await {
                    let coords = position.coords;
                    web_sys::console::log_1(&format!("Got coordinates: {}, {}", coords.latitude, coords.longitude).into());
                    
                    if let Ok(map_instance) = L::render_map(map_id, &coords) {
                        map.set(Some(map_instance));
                        web_sys::console::log_1(&"Map created".into());

                        if let Some(map_ref) = &*map {
                            web_sys::console::log_1(&"Adding marker...".into());
                            match map_ref.add_leaflet_marker(&coords) {
                                Ok(marker_instance) => {
                                    marker.set(Some(marker_instance));
                                    web_sys::console::log_1(&"Marker added successfully".into());
                                }
                                Err(e) => {
                                    web_sys::console::error_1(&format!("Error adding marker: {:?}", e).into());
                                }
                            }
                        }

                        if let Ok(location) = NominatimLookup::reverse(coords).await {
                            location_name.set(location.display_name().to_string());
                        }
                    }
                }
            });
            || ()
        });
    }

    // Handle incoming Nostr events
    {
        let marker = marker.clone();
        use_effect_with(relay_ctx.unique_notes.clone(), move |notes| {
            if let Some(note) = notes.last() {
                if note.get_kind() == 27235 {
                    if let Ok(coords) = serde_json::from_str::<GeolocationCoordinates>(&note.get_content()) {
                        if let Some(marker_ref) = &*marker {
                            let _ = marker_ref.set_lat_lng(&coords.into());
                        }
                    }
                }
            }
            || {}
        });
    }

    // Cleanup subscription on unmount
    {
        let unsubscriber = relay_ctx.unsubscribe.clone();
        let sub_id = (*subscription_id).clone();
        
        use_effect_with((), move |_| {
            move || {
                if let Some(id) = sub_id {
                    unsubscriber.emit(id);
                }
            }
        });
    }


    html! {
        <div class="flex flex-col gap-4 w-full">
            <div 
                id={map_id} 
                style="height: 500px; width: 100%; position: relative;"
                class="rounded-lg shadow-md" 
            />
            if !location_name.is_empty() {
                <div class="text-sm text-gray-600">
                    {"Current location: "}{&*location_name}
                </div>
            }
        </div>
    }
}