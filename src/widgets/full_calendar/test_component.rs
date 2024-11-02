use yew::prelude::*;
use crate::relay_pool::relay_pool::NostrProps;
use super::{FullCalendarComponent, FullCalendarEvent};
use js_sys::Date;
use nostro2::notes::SignedNote;
use nostro2::relays::{NostrFilter, NostrSubscription};
use serde_json::json;
use wasm_bindgen::JsValue;
use crate::widgets::toastify::ToastifyOptions;

#[function_component(FullCalendarTest)]
pub fn calendar_test() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let events = use_state(Vec::new);
    // Set up subscription for calendar events
    {
        let relay_ctx = relay_ctx.clone();
        use_effect_with((), move |_| {
            // Create and configure filter for calendar events
            let filter = NostrFilter::default()
                .new_kinds(vec![31924])
                .new_limit(50);
            
            // Create and send subscription
            let subscription = NostrSubscription::new(filter);
            relay_ctx.subscribe.emit(subscription);
            || ()
        });
    }

    // Convert notes to calendar events
    let convert_note_to_event = |note: &SignedNote| -> Option<FullCalendarEvent> {
        gloo::console::log!("Processing note:", note.get_kind());
        if note.get_kind() == 31924 {
            gloo::console::log!("Found calendar note:", note.get_content());
            if let Ok(content) = serde_json::from_str::<serde_json::Value>(&note.get_content()) {
                gloo::console::log!("Parsed content:", content.to_string());
                
                let start_str = content["start"].as_str()?;
                let end_str = content["end"].as_str()?;
                gloo::console::log!("Start:", start_str, "End:", end_str);
                
                let start = Date::new(&JsValue::from_str(start_str));
                let end = Date::new(&JsValue::from_str(end_str));
                let title = content["title"].as_str()?;
    
                let event = FullCalendarEvent::new(
                    &note.get_id().to_string(),
                    title,
                    start,
                    end,
                    FullCalendarEvent::COLOR_BLUE,
                    json!({
                        "noteId": note.get_id().to_string(),
                        "pubkey": note.get_pubkey().to_string(),
                        "kind": 31924,
                    })
                );
                gloo::console::log!("Created event:", event.get_title());
                Some(event)
            } else {
                gloo::console::log!("Failed to parse note content");
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
        // Convert JsString to String
        let start_time = start.to_locale_time_string("en-US")
            .as_string()
            .unwrap_or_default();
        
        let event_title = format!("Event at {}", start_time);
        
        let start_str = start.to_iso_string().as_string().unwrap_or_default();
        let end_str = end.to_iso_string().as_string().unwrap_or_default();

        let content = json!({
            "title": event_title,
            "start": start_str,
            "end": end_str,
            "type": "calendar_event",
            "backgroundColor": FullCalendarEvent::COLOR_BLUE,
            "textColor": "#ffffff",
            "allDay": false,
            "timeFormat": "h:mm a"
        });

        gloo::console::log!("Event content:", content.to_string());

        let new_keys = nostro2::userkeys::UserKeys::generate();
        let new_note = nostro2::notes::Note::new(
            &new_keys.get_public_key(),
            31924,
            &content.to_string()
        );
        let signed_note = new_keys.sign_nostr_event(new_note);
        
        gloo::console::log!("Sending note to relay:", signed_note.get_id().to_string());
        
        relay_ctx.send_note.emit(signed_note);

        ToastifyOptions::new_success("Created new calendar event").show();
    })
};

    // Update events when notes change
    {
        let events = events.clone();
        let notes = relay_ctx.unique_notes.clone();
        
        use_effect_with(notes, move |notes| {
            gloo::console::log!("Received notes update, total notes:", notes.len());
            let calendar_events: Vec<FullCalendarEvent> = notes
                .iter()
                .filter_map(|note| {
                    if note.get_kind() == 31924 {
                        gloo::console::log!("Processing calendar note:", note.get_id().to_string());
                    }
                    convert_note_to_event(note)
                })
                .collect();
            
            gloo::console::log!("Created calendar events:", calendar_events.len());
            events.set(calendar_events);
            || ()
        });
    }
    let events_debug = (*events).clone();
    gloo::console::log!("Rendering with events:", events_debug.len());

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
                events={events_debug}  // Use our debug copy
                on_event_click={handle_event_click}
                on_date_select={handle_date_select}
                class={classes!("rounded-lg", "shadow-lg", "bg-white")}
        />
        </div>
    }
}