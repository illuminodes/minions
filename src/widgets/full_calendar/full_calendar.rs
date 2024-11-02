use js_sys::{Function, Object, Reflect};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::Element;

use js_sys::Date;

#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    pub type CalendarEvent;
    #[wasm_bindgen(method, js_name = remove)]
    pub fn remove(this: &CalendarEvent);
}
#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone, PartialEq)]
    pub type Calendar;
    #[wasm_bindgen(constructor, js_namespace = FullCalendar, js_name = Calendar)]
    pub fn new(calendar_element: &Element, options: JsValue) -> Calendar;
    #[wasm_bindgen(method, js_name = render)]
    pub fn render(this: &Calendar);
    #[wasm_bindgen(method, js_name = addEvent)]
    fn add_event(this: &Calendar, event: JsValue);
    #[wasm_bindgen(method, js_name = getEventById)]
    pub fn get_event_by_id(this: &Calendar, id: &str) -> Option<CalendarEvent>;
    #[wasm_bindgen(method, js_name = getEvents)]
    pub fn get_events(this: &Calendar) -> Vec<CalendarEvent>;
    #[wasm_bindgen(method, js_name = changeView)]
    pub fn change_view(this: &Calendar, view: &str, date: &str);
}
impl Calendar {
    pub fn add_calendar_event(&self, event: FullCalendarEvent) -> Result<(), JsValue> {
        let js_value = serde_wasm_bindgen::to_value(&event)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert event: {}", e)))?;
        self.add_event(js_value);
        Ok(())
    }

    pub fn add_or_replace_event(&self, event: FullCalendarEvent) -> Result<(), JsValue> {
        if let Some(old_event) = self.get_event_by_id(&event.get_id()) {
            old_event.remove();
        }
        self.add_calendar_event(event)
    }

    pub fn remove_event(&self, id: &str) {
        if let Some(event) = self.get_event_by_id(id) {
            event.remove();
        }
    }

    pub fn clear_events(&self) {
        let events = self.get_events();
        for event in events {
            event.remove();
        }
    }

    pub fn update_events(&self, events: Vec<FullCalendarEvent>) {
        self.clear_events();
        for event in events {
            if let Err(e) = self.add_calendar_event(event) {
                gloo::console::error!("Failed to add calendar event:", e);
            }
        }
    }

    // View Management Methods
    pub fn reload(&self) {
        self.render();
    }
    
    pub fn update_view(&self, view_name: &str) {
        let current_date = js_sys::Date::new_0();
        let date_str = current_date.to_iso_string().as_string().unwrap_or_default();
        self.change_view(view_name, &date_str);
    }
    
    pub fn update_view_to_date(&self, view_name: &str, date: &js_sys::Date) {
        let date_str = date.to_iso_string().as_string().unwrap_or_default();
        self.change_view(view_name, &date_str);
    }

    // Batch Operations
    pub fn batch_update<F>(&self, f: F) 
    where 
        F: FnOnce(&Self)
    {
        f(self);
        self.render();
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCalendarHeaderOptions {
    start: &'static str,
    center: &'static str,
    end: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTimeFormat {
    pub hour: &'static str,
    pub minute: &'static str,
    pub meridiem: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCalendarOptions {
    #[serde(rename = "initialView")]
    pub intial_view: &'static str,
    pub locale: &'static str,
    #[serde(rename = "expandRows")]
    pub expand_rows: bool,
    #[serde(rename = "allDaySlot")]
    pub all_day_slot: bool,
    pub selectable: bool,
    #[serde(rename = "firstDay")]
    pub first_day: u32,
    #[serde(rename = "slotDuration")]
    pub slot_duration: String,
    #[serde(rename = "headerToolbar")]
    pub header_toolbar: FullCalendarHeaderOptions,
    #[serde(rename = "selectLongPressDelay")]
    pub select_long_press_delay: u32,
    #[serde(skip)]
    event_click_handler: Option<Function>,
    #[serde(skip)]
    select_handler: Option<Function>,
    #[serde(skip)]
    date_click_handler: Option<Function>,
    #[serde(rename = "displayEventTime")]
    pub display_event_time: bool,
    #[serde(rename = "eventTimeFormat")]
    pub event_time_format: EventTimeFormat,
}
impl FullCalendarOptions {
    pub fn new() -> Self {
        Self {
            intial_view: "dayGridMonth",
            locale: "en",
            expand_rows: true,
            all_day_slot: true,
            selectable: true,
            first_day: 0,
            slot_duration: "00:30:00".to_string(),
            header_toolbar: FullCalendarHeaderOptions {
                start: "prev,next today",
                center: "title",
                end: "dayGridMonth,timeGridWeek,timeGridDay",
            },
            select_long_press_delay: 250,
            event_click_handler: None,
            select_handler: None,
            date_click_handler: None,
            display_event_time: true,
            event_time_format: EventTimeFormat {
                hour: "numeric",
                minute: "2-digit",
                meridiem: "short",
            },
        }
    }

    pub fn with_event_click<F>(mut self, f: F) -> Self 
    where
        F: Fn(JsValue) + 'static,
    {
        let closure = Closure::wrap(Box::new(f) as Box<dyn Fn(JsValue)>);
        self.event_click_handler = Some(closure.into_js_value().unchecked_into());
        self
    }

    pub fn with_select<F>(mut self, f: F) -> Self
    where
        F: Fn(JsValue) + 'static,
    {
        let closure = Closure::wrap(Box::new(f) as Box<dyn Fn(JsValue)>);
        self.select_handler = Some(closure.into_js_value().unchecked_into());
        self
    }

    pub fn with_date_click<F>(mut self, f: F) -> Self
    where
        F: Fn(JsValue) + 'static,
    {
        let closure = Closure::wrap(Box::new(f) as Box<dyn Fn(JsValue)>);
        self.date_click_handler = Some(closure.into_js_value().unchecked_into());
        self
    }
    fn validate_handler(&self) -> Result<(), JsValue> {
        let validate_fn = |handler: &Function, name: &str| -> Result<(), JsValue> {
            if js_sys::Reflect::get(handler, &"call".into())?.is_undefined() {
                return Err(JsValue::from_str(&format!("Invalid {} handler", name)));
            }
            Ok(())
        };

        if let Some(handler) = &self.event_click_handler {
            validate_fn(handler, "event click")?;
        }
        if let Some(handler) = &self.select_handler {
            validate_fn(handler, "select")?;
        }
        if let Some(handler) = &self.date_click_handler {
            validate_fn(handler, "date click")?;
        }
        Ok(())
    }

    pub fn to_js_value(&self) -> Result<JsValue, JsValue> {
        self.validate_handler()?;
        
        let obj = js_sys::Object::new();
        
        // Convert the basic options
        let base_options = serde_wasm_bindgen::to_value(&self)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert options: {}", e)))?;
        
        // Copy properties
        let base_obj: Object = base_options.into();
        let keys = js_sys::Object::keys(&base_obj);
        for i in 0..keys.length() {
            let key = keys.get(i);
            let value = js_sys::Reflect::get(&base_obj, &key)?;
            js_sys::Reflect::set(&obj, &key, &value)?;
        }
        
        // Add handlers
        if let Some(handler) = &self.event_click_handler {
            js_sys::Reflect::set(&obj, &JsValue::from_str("eventClick"), handler)?;
        }
        if let Some(handler) = &self.select_handler {
            js_sys::Reflect::set(&obj, &JsValue::from_str("select"), handler)?;
        }
        if let Some(handler) = &self.date_click_handler {
            js_sys::Reflect::set(&obj, &JsValue::from_str("dateClick"), handler)?;
        }
        
        Ok(obj.into())
    }
}
impl Default for FullCalendarHeaderOptions {
    fn default() -> Self {
        Self {
            start: "prev,next today",
            center: "title",
            end: "dayGridMonth,timeGridWeek,timeGridDay",
        }
    }
}

impl Default for FullCalendarOptions {
    fn default() -> Self {
        Self::new()
    }
}
impl Into<JsValue> for FullCalendarOptions {
    fn into(self) -> JsValue {
        self.to_js_value().unwrap_or_else(|e| {
            gloo::console::error!("Failed to convert calendar options:", e);
            JsValue::NULL
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullCalendarSelectEvent {
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub start: Date,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub end: Date,
    #[serde(rename = "startStr")]
    pub start_str: String,
    #[serde(rename = "endStr")]
    pub end_str: String,
}

impl TryFrom<JsValue> for FullCalendarSelectEvent {
    type Error = JsValue;
    
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert select event: {}", e)))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullCalendarEvent {
    id: String,
    title: String,
    #[serde(rename = "allDay")]
    all_day: bool,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    start: js_sys::Date,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    end: js_sys::Date,
    #[serde(rename = "startStr")]
    start_str: String,
    #[serde(rename = "endStr")]
    end_str: String,
    #[serde(rename = "backgroundColor")]
    background_color: String,
    #[serde(rename = "extendedProps")]
    extended_props: Value,
    display: String,
}
impl FullCalendarEvent {
    // Add standard color constants
    pub const COLOR_BLUE: &'static str = "#3788d8";
    pub const COLOR_GREEN: &'static str = "#2ecc71";
    pub const COLOR_RED: &'static str = "#e74c3c";
    pub const COLOR_YELLOW: &'static str = "#f1c40f";
    pub const COLOR_PURPLE: &'static str = "#9b59b6";
    
    // Add a builder-style method for setting color
    pub fn with_color(mut self, color: &str) -> Self {
        self.background_color = color.to_string();
        self
    }

    // Add a builder-style method for setting all_day
    pub fn with_all_day(mut self, all_day: bool) -> Self {
        self.all_day = all_day;
        self
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    pub fn get_start_str(&self) -> &str {
        &self.start_str
    }

    pub fn get_end_str(&self) -> &str {
        &self.end_str
    }

    pub fn quick_event(id: &str, title: &str, start: Date, duration_mins: i32) -> Self {
        let end = Date::new(&start.clone().into());
        let current_minutes = end.get_minutes() as u32;
        let total_minutes = current_minutes + duration_mins as u32;
        end.set_minutes(total_minutes);
        
        Self::new(
            id,
            title,
            start,
            end,
            Self::COLOR_BLUE,
            serde_json::Value::Null
        )
    }

    pub fn from_event_value(value: JsValue) -> Result<Self, JsValue> {
        let js_object: Object = value.dyn_into()?;
        let event_object = Reflect::get(&js_object, &JsValue::from_str("event"))?;
        serde_wasm_bindgen::from_value(event_object)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert event: {}", e)))
    }

    pub fn from_event(event: &web_sys::Event) -> Result<Self, JsValue> {
        let js_value: JsValue = event.into();
        let js_object: Object = js_value.dyn_into()?;
        let event_object = Reflect::get(&js_object, &JsValue::from_str("event"))?;
        Self::try_from(event_object)
    }

    pub fn new(
        id: &str,
        title: &str,
        start: Date,
        end: Date,
        color: &str,
        extended_props: Value,
    ) -> Self {
        let locale_options: JsValue = js_sys::Object::new().into();

        FullCalendarEvent {
            id: id.to_string(),
            title: title.to_string(),
            all_day: false,
            start: start.clone(),
            end: end.clone(),
            background_color: color.to_string(),
            start_str: start.to_locale_string("es-ES", &locale_options).into(),
            end_str: end.to_locale_string("es-ES", &locale_options).into(),
            display: "block".to_string(),
            extended_props,
        }
    }
    pub fn new_background_event(
        id: &str,
        title: &str,
        start: Date,
        end: Date,
        color: &str,
        extended_props: Value,
    ) -> Self {
        let locale_options: JsValue = js_sys::Object::new().into();

        FullCalendarEvent {
            id: id.to_string(),
            title: title.to_string(),
            all_day: false,
            start: start.clone(),
            end: end.clone(),
            background_color: color.to_string(),
            start_str: start.to_locale_string("es-ES", &locale_options).into(),
            end_str: end.to_locale_string("es-ES", &locale_options).into(),
            display: "background".to_string(),
            extended_props,
        }
    }
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    pub fn get_dates(&self) -> (Date, Date) {
        (self.start.clone(), self.end.clone())
    }
}
impl Into<JsValue> for FullCalendarEvent {
    fn into(self) -> JsValue {
        serde_wasm_bindgen::to_value(&self).expect("Failed to convert FullCalendarEvent to JsValue")
    }
}
// Add TryFrom for better error handling
impl TryFrom<JsValue> for FullCalendarEvent {
    type Error = JsValue;
    
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert calendar event: {}", e)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullCalendarDateClickInfo {
    #[serde(with = "serde_wasm_bindgen::preserve")]
    date: Date,
    #[serde(rename = "dateStr")]
    date_str: String,
}
impl FullCalendarDateClickInfo {
    pub fn date_str(&self) -> String {
        self.date_str.clone()
    }
}

impl Into<JsValue> for FullCalendarDateClickInfo {
    fn into(self) -> JsValue {
        serde_wasm_bindgen::to_value(&self)
            .expect("Failed to convert FullCalendarDateClickInfo to JsValue")
    }
}

impl TryFrom<JsValue> for FullCalendarDateClickInfo {
    type Error = JsValue;
    
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value)
            .map_err(|e| JsValue::from_str(&format!("Failed to convert date click info: {}", e)))
    }
}
