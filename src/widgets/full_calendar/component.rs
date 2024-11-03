use yew::prelude::*;
use web_sys::HtmlElement;
use super::full_calendar::{Calendar, FullCalendarEvent, FullCalendarOptions, FullCalendarSelectEvent};
use js_sys::Date;
use wasm_bindgen::JsValue;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub calendar_id: AttrValue,
    #[prop_or_default]
    pub events: Vec<FullCalendarEvent>,
    #[prop_or_default]
    pub on_event_click: Option<Callback<FullCalendarEvent>>,
    #[prop_or_default]
    pub on_date_select: Option<Callback<(Date, Date)>>,
    #[prop_or_default]
    pub on_calendar_created: Option<Callback<Calendar>>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(FullCalendarComponent)]
pub fn calendar_component(props: &Props) -> Html {
    let calendar_ref = use_node_ref();
    let calendar = use_state(|| None::<Calendar>);

    // Initialize calendar
    {
        let calendar = calendar.clone();
        let calendar_ref = calendar_ref.clone();
        let events = props.events.clone();
        let on_calendar_created = props.on_calendar_created.clone();
        let on_event_click = props.on_event_click.clone();
        let on_date_select = props.on_date_select.clone();

        use_effect_with((), move |_| {
            if let Some(element) = calendar_ref.cast::<HtmlElement>() {
                // Create options and add handlers
                let options = {
                    let mut opt = FullCalendarOptions::new();
                    
                    // Add event click handler if callback provided
                    if let Some(event_cb) = on_event_click {
                        opt = opt.with_event_click(move |event_value: JsValue| {
                            if let Ok(cal_event) = FullCalendarEvent::from_event_value(event_value) {
                                event_cb.emit(cal_event);
                            }
                        });
                    }

                    // Add date selection handler if callback provided
                    if let Some(select_cb) = on_date_select {
                        opt = opt.with_select(move |select_value: JsValue| {
                            if let Ok(select_event) = FullCalendarSelectEvent::try_from(select_value) {
                                select_cb.emit((select_event.start, select_event.end));
                            }
                        });
                    }
                    
                    opt
                };

                let calendar_instance = Calendar::new(&element, options.into());
                
                // Add initial events with error handling
                for event in events {
                    if let Err(e) = calendar_instance.add_or_replace_event(event) {
                        gloo::console::error!("Failed to add event:", e);
                    }
                }

                calendar_instance.render();
                
                if let Some(cb) = on_calendar_created {
                    cb.emit(calendar_instance.clone());
                }
                
                calendar.set(Some(calendar_instance));
            }
            || ()
        });
    }

    // Update events when props change
    {
        let calendar = calendar.clone();
        let events = props.events.clone();
        
        use_effect_with(events, move |events| {
            if let Some(calendar_instance) = (*calendar).clone() {
                calendar_instance.clear_events();
                for event in events {
                    if let Err(e) = calendar_instance.add_or_replace_event(event.clone()) {
                        gloo::console::error!("Failed to add/replace event:", e);
                    }
                }
            }
            || ()
        });
    }

    html! {
        <div 
            ref={calendar_ref} 
            class={classes!("full-calendar-container", props.class.clone())}
            style="height: 600px;"
        />
    }
}