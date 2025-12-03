use nostr_minions::{
    // nostro2::{NostrNote, NostrSigner, NostrSubscription},
    NostrAppProvider,
};
use yew::prelude::*;

#[wasm_bindgen_test::wasm_bindgen_test]
pub fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    let relays = vec![
        nostr_minions::UserRelay {
            url: "wss://relay.illuminodes.com".to_string(),
            read: true,
            write: true,
        },
        nostr_minions::UserRelay {
            url: "wss://relay.arrakis.lat".to_string(),
            read: true,
            write: true,
        },
    ];
    html! {
        <NostrAppProvider {relays} fallback={html!(<Splash/>)}>
                <FullCalendarTest />
                // ADD NEW TEST COMPONENTS HERE WITH INLINES
                // <nostr_minions::RelayPoolTest />
                // <nostr_minions::NostrIdLoginTest />
        </NostrAppProvider>
    }
}

#[function_component(Splash)]
fn relay_pool_test() -> Html {
    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center">
       </div>
    }
}

use web_sys::{js_sys::Date, wasm_bindgen::JsValue};
use yew_full_calendar::{Calendar, EventDate, FullCalendarComponent};

#[function_component(FullCalendarTest)]
pub fn calendar_test() -> Html {
    let on_calendar_created = {
        Callback::from(move |calendar: Calendar| {
            web_sys::console::log_1(&format!("Calendar created: {calendar:?}").into());
            // calendar_state.set(Some(calendar));
            let calendar_clone = calendar.clone();
            calendar.batch(move || {
                for i in 0..4 {
                    let start = Date::new_0();
                    let end = Date::new(&JsValue::from_f64(start.get_time() + 3600. * 1000.));
                    #[derive(serde::Serialize, serde::Deserialize)]
                    struct ExtendedPropsTest {
                        id: u32,
                        title: String,
                    }
                    let builder = yew_full_calendar::EventBuilder::default()
                        .id(i.to_string().as_str())
                        .start(EventDate::DateObject(start))
                        .end(EventDate::DateObject(end))
                        .color("hsl(120, 100%, 50%)")
                        .interactive(true)
                        .extended_props(ExtendedPropsTest {
                            id: i,
                            title: format!("Event {i}"),
                        })
                        .unwrap()
                        .all_day(false);
                    calendar_clone.add_or_replace_event(builder).expect("add event");
                    web_sys::console::log_1(&"Added event".into());
                }
            });
        })
    };

    let on_event_click = {
        Callback::from(move |event: yew_full_calendar::EventClickInfo| {
            #[derive(serde::Serialize, serde::Deserialize, Debug)]
            struct ExtendedPropsTest {
                id: u32,
                title: String,
            }
            let Some(event) = event.event() else {
                return;
            };
            let props = event.extended_props::<ExtendedPropsTest>().unwrap();
            web_sys::console::log_1(&format!("Event props: {props:#?}").into());
        })
    };

    let on_date_select = {
        Callback::from(move |select_event: yew_full_calendar::SelectionInfo| {
            web_sys::console::log_1(&format!("Date selected: {select_event:?}").into());
        })
    };

    let on_dates_set = {
        Callback::from(move |view: yew_full_calendar::DateSetEvent| {
            web_sys::console::log_1(&format!("View changed: {view:#?}").into());
        })
    };

    let calendar_options = yew_full_calendar::Options::new()
        .with_initial_view(yew_full_calendar::InitialView::TimeGridWeek)
        .with_locale(yew_full_calendar::Locale::Es)
        .with_selectable(true);

    html! {
        <>
        <h1>{"Calendar"}</h1>
            <FullCalendarComponent
                calendar_id="full-calendar"
                {calendar_options}
                // {events}  // Use our debug copy
                {on_event_click}
                {on_date_select}
                {on_calendar_created}
                {on_dates_set}
        /></>
    }
}
