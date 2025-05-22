use super::{Calendar, FullCalendarComponent, FullCalendarEvent};
use crate::relay_pool::NostrRelayPoolStore;
use crate::widgets::toastify::ToastifyOptions;
use nostro2_signer::nostro2::NostrSigner;
use nostro2::NostrNote;
use nostro2::NostrSubscription;
use serde_json::json;
use wasm_bindgen::JsValue;
use web_sys::js_sys::Date;
use yew::prelude::*;

#[function_component(FullCalendarTest)]
pub fn calendar_test() -> Html {
    let relay_ctx = use_context::<NostrRelayPoolStore>().expect("No relay context found");
    let events = use_state(Vec::new);
    // Set up subscription for calendar events
    {
        let ctx = relay_ctx.clone();
        use_effect_with((), move |()| {
            // Create and configure filter for calendar events
            let filter = NostrSubscription {
                kinds: Some(vec![31924]),
                limit: Some(50),
                ..Default::default()
            };

            // Create and send subscription
            ctx.send(filter);
            || ()
        });
    }

    // Convert notes to calendar events
    let convert_note_to_event = |note: &NostrNote| -> Option<FullCalendarEvent> {
        gloo::console::log!("Processing note:", note.kind);
        if note.kind == 31924 {
            gloo::console::log!("Found calendar note:", note.content.as_str());
            if let Ok(content) = serde_json::from_str::<serde_json::Value>(&note.content) {
                gloo::console::log!("Parsed content:", content.to_string());

                let start_str = content["start"].as_str()?;
                let end_str = content["end"].as_str()?;
                gloo::console::log!("Start:", start_str, "End:", end_str);

                let start = Date::new(&JsValue::from_str(start_str));
                let end = Date::new(&JsValue::from_str(end_str));
                let title = content["title"].as_str()?;

                let event = FullCalendarEvent::new(
                    note.id.as_ref().unwrap().as_str(),
                    title,
                    &start,
                    &end,
                    FullCalendarEvent::COLOR_BLUE,
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
            let start_time = start
                .to_locale_time_string("en-US")
                .as_string()
                .unwrap_or_default();

            let event_title = format!("Event at {start_time}");

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

            let new_keys = nostro2_signer::keypair::NostrKeypair::generate(false);
            let mut new_note = NostrNote {
                pubkey: new_keys.public_key(),
                kind: 31924,
                content: content.to_string(),
                ..Default::default()
            };
            if new_keys.sign_nostr_note(&mut new_note).is_ok() {
                gloo::console::log!(
                    "Sending note to relay:",
                    new_note.id.as_ref().unwrap().as_str()
                );

                relay_ctx.send(new_note);

                ToastifyOptions::new_success("Created new calendar event").show();
            } else {
                gloo::console::log!("Failed to sign note");
                ToastifyOptions::new_relay_error("Failed to create event").show();
            }
        })
    };

    // Update events when notes change
    {
        let events = events.clone();

        use_effect_with(relay_ctx.unique_notes.clone(), move |notes| {
            if let Some(last_note) = notes.last() {
                let mut new_events = (*events).clone();
                if let Some(event) = convert_note_to_event(last_note) {
                    if last_note.kind == 31924 {
                        new_events.push(event);
                        events.set(new_events);
                    }
                }
            }
            || ()
        });
    }
    let events_debug = events;
    gloo::console::log!("Rendering with events:", events_debug.len());
    let calendar_state = use_state(|| None);
    let on_calendar_created = {
        let calendar_state = calendar_state;
        Callback::from(move |calendar: Calendar| {
            calendar_state.set(Some(calendar));
        })
    };

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
                {on_calendar_created}
                class={classes!("rounded-lg", "shadow-lg", "bg-white", "h-32", "h-96")}
        />
        </div>
    }
}
