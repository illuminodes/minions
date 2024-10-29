use yew::prelude::*;
use super::component::LeafletComponent;
use crate::browser_api::geolocation::GeolocationCoordinates;
use crate::relay_pool::relay_pool::NostrProps;
use crate::widgets::leaflet::LeafletMap;
use crate::widgets::leaflet::LeafletLocateOptions;

#[function_component(LeafletTest)]
pub fn leaflet_test() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let map = use_state(|| None::<LeafletMap>);

    let send_test_event = {
        let note_sender = relay_ctx.send_note.clone();
        Callback::from(move |_| {
            let new_keys = nostro2::userkeys::UserKeys::generate();
            let coords = GeolocationCoordinates {
                latitude: 28.4089,
                longitude: 76.9699,
                accuracy: 10.0,
                altitude: None,
                altitude_accuracy: None,
                speed: None,
            };
            
            let content = serde_json::to_string(&coords).unwrap();
            let new_note = nostro2::notes::Note::new(
                &new_keys.get_public_key(),
                27235,
                &content
            );
            let signed_note = new_keys.sign_nostr_event(new_note);
            note_sender.emit(signed_note);
            crate::widgets::toastify::ToastifyOptions::new_event_received("Test location sent").show();
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

    // Geolocation
    let start_locate = {
        let map = map.clone();
        Callback::from(move |_| {
            if let Some(map_ref) = &*map {
                let options = LeafletLocateOptions {
                    watch: true,
                    set_view: true,
                    max_zoom: 16.0,
                    timeout: 10000,
                    maximum_age: 0,
                    enable_high_accuracy: true,
                };
                map_ref.start_locate(Some(options));
                crate::widgets::toastify::ToastifyOptions::new_event_received("Started location tracking").show();
            }
        })
    };

    let stop_locate = {
        let map = map.clone();
        Callback::from(move |_| {
            if let Some(map_ref) = &*map {
                map_ref.stop_location_watch();
                crate::widgets::toastify::ToastifyOptions::new_event_received("Stopped location tracking").show();
            }
        })
    };

    html! {
        <div class="flex flex-col gap-4 p-4">
            <h1 class="text-2xl font-bold">{"Leaflet Map Test"}</h1>
            <LeafletComponent 
                on_map_created={Callback::from({
                    let map = map.clone();
                    move |map_instance: LeafletMap| map.set(Some(map_instance))
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

                // Geolocation buttons
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
