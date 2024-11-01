use yew::prelude::*;
use crate::relay_pool::relay_pool::NostrProps;
use super::{FullCalendarComponent, FullCalendarEvent};
use js_sys::Date;
use nostro2::notes::SignedNote;
use serde_json::json;
use wasm_bindgen::JsValue;
use crate::widgets::toastify::ToastifyOptions;

#[function_component(FullCalendarTest)]
pub fn calendar_test() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let events = use_state(Vec::new);

    // Convert notes to calendar events
    let convert_note_to_event = |note: &SignedNote| -> Option<FullCalendarEvent> {
        if note.get_kind() == 31924 {
            if let Ok(content) = serde_json::from_str::<serde_json::Value>(&note.get_content()) {
                // Try to parse start and end times
                let start = Date::new(&JsValue::from_str(&content["start"].as_str()?));
                let end = Date::new(&JsValue::from_str(&content["end"].as_str()?));
                let title = content["title"].as_str()?;

                Some(FullCalendarEvent::new(
                    &note.get_id().to_string(),
                    title,
                    start,
                    end,
                    FullCalendarEvent::COLOR_BLUE,  // Use the constant from FullCalendarEvent
                    json!({
                        "noteId": note.get_id().to_string(),
                        "pubkey": note.get_pubkey().to_string(),
                        "kind": 31924,
                    })
                ))
            } else {
                None
            }
        } else {
            None
        }
    };

    // Handle event click
    let handle_event_click = {
        Callback::from(move |event: FullCalendarEvent| {
            // Show event details in a toast
            let toast_msg = format!(
                "Event: {} \nTime: {} - {}", 
                event.get_title(),
                event.get_start_str(),
                event.get_end_str()
            );
            ToastifyOptions::new_event_received(&toast_msg).show();
        })
    };

// Handle date selection for new events
let handle_date_select = {
    let relay_ctx = relay_ctx.clone();
    Callback::from(move |(start, end): (Date, Date)| {
        let event_title = "New Calendar Event";
        let content = json!({
            "title": event_title,
            "start": start.to_iso_string().as_string().unwrap(),  // Convert to String
            "end": end.to_iso_string().as_string().unwrap(),      // Convert to String
            "type": "calendar_event"
        });

        // Create and sign new note
        let new_keys = nostro2::userkeys::UserKeys::generate();
        let new_note = nostro2::notes::Note::new(
            &new_keys.get_public_key(),
            31924,
            &content.to_string()
        );
        let signed_note = new_keys.sign_nostr_event(new_note);
        relay_ctx.send_note.emit(signed_note);

        ToastifyOptions::new_success("Created new calendar event").show();
    })
};

    // Update events when notes change
    {
        let events = events.clone();
        let notes = relay_ctx.unique_notes.clone();
        
        use_effect_with(notes, move |notes| {
            let calendar_events: Vec<FullCalendarEvent> = notes
                .iter()
                .filter_map(|note| convert_note_to_event(note))
                .collect();
            
            events.set(calendar_events);
            || ()
        });
    }

    html! {
        <div class="flex flex-col gap-4 p-4">
            <h2 class="text-2xl font-bold">{"Calendar Events"}</h2>
            <div class="flex flex-col gap-2">
                <p class="text-gray-600">{"Create events by clicking and dragging on the calendar."}</p>
                <p class="text-gray-600">{"Click an event to view details."}</p>
                <p class="text-sm text-gray-500">{"All events are stored as Nostr notes (kind: 31924)"}</p>
            </div>
            <FullCalendarComponent
                calendar_id="full-calendar"
                events={(*events).clone()}
                on_event_click={handle_event_click}
                on_date_select={handle_date_select}
                class={classes!("rounded-lg", "shadow-lg", "bg-white")}
            />
        </div>
    }
}