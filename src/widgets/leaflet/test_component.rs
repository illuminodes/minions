use yew::prelude::*;
use super::component::LeafletComponent;
use crate::browser_api::geolocation::GeolocationCoordinates;
use crate::relay_pool::relay_pool::NostrProps;

#[function_component(LeafletTest)]
pub fn leaflet_test() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");

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

    html! {
        <div class="flex flex-col gap-4 p-4">
            <h1 class="text-2xl font-bold">{"Leaflet Map Test"}</h1>
            <LeafletComponent />
            <div class="flex gap-2">
                <button 
                    onclick={send_test_event}
                    class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                >
                    {"Send Test Location"}
                </button>
            </div>
        </div>
    }
}
