use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Serialize)]
pub struct ToastifyOptions {
    text: String,
    duration: u32,
    close: bool,
    gravity: &'static str,  // `top` or `bottom`
    position: &'static str, // `left`, `center` or `right`
    #[serde(rename = "stopOnFocus")]
    stop_on_focus: bool, // Prevents dismissing of toast on hover
    #[serde(rename = "className")]
    class_name: &'static str,
    #[serde(rename = "style")]
    style: Option<String>,
}

impl ToastifyOptions {
    pub fn show(self) {
        let options: JsValue = self.into();
        toasts(&options).show_toast();
    }

    // Add new methods for relay events
    pub fn new_relay_connected(relay_url: &str) -> Self {
        ToastifyOptions {
            text: format!("Connected to relay: {}", relay_url),
            duration: 3000,
            close: true,
            gravity: "top",
            position: "right",
            stop_on_focus: true,
            class_name: "relay-success-toast",
            style: Some("background: linear-gradient(to right, #00b09b, #96c93d)".to_string()),
        }
    }

    pub fn new_relay_disconnected(relay_url: &str) -> Self {
        ToastifyOptions {
            text: format!("Disconnected from relay: {}", relay_url),
            duration: 4000,
            close: true,
            gravity: "top",
            position: "right",
            stop_on_focus: true,
            class_name: "relay-error-toast",
            style: Some("background: linear-gradient(to right, #ff5f6d, #ffc371)".to_string()),
        }
    }

    pub fn new_event_received(event_type: &str) -> Self {
        ToastifyOptions {
            text: format!("New {} event received", event_type),
            duration: 2000,
            close: true,
            gravity: "top",
            position: "right",
            stop_on_focus: true,
            class_name: "event-toast",
            style: Some("background: linear-gradient(to right, #2193b0, #6dd5ed)".to_string()),
        }
    }

    pub fn new_relay_error(error: &str) -> Self {
        ToastifyOptions {
            text: format!("Relay error: {}", error),
            duration: 4000,
            close: true,
            gravity: "top",
            position: "right",
            stop_on_focus: true,
            class_name: "relay-error-toast",
            style: Some("background: linear-gradient(to right, #cb2d3e, #ef473a)".to_string()),
        }
    }

    pub fn new_login(text: String) -> Self {
        ToastifyOptions {
            text,
            duration: 21000,
            close: true,
            gravity: "top",
            position: "left",
            stop_on_focus: true,
            class_name: "success-toast",
            style: Some("background: linear-gradient(to right, #00b09b, #96c93d)".to_string()),
        }
    }

    pub fn new_success(text: &'static str) -> Self {
        ToastifyOptions {
            text: text.to_string(),
            duration: 2100,
            close: true,
            gravity: "top",
            position: "left",
            stop_on_focus: true,
            class_name: "success-toast",
            style: Some("background: linear-gradient(to right, #00b09b, #96c93d)".to_string()),
        }
    }

    pub fn new_failure(text: &'static str) -> Self {
        ToastifyOptions {
            text: text.to_string(),
            duration: 2100,
            close: true,
            gravity: "top",
            position: "left",
            stop_on_focus: true,
            class_name: "failure-toast",
            style: Some("background: linear-gradient(to right, #ff5f6d, #ffc371)".to_string()),
        }
    }
}

impl Into<JsValue> for ToastifyOptions {
    fn into(self) -> JsValue {
        serde_wasm_bindgen::to_value(&self).expect("Failed to serialize Toast")
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    pub type Toastify;
    #[wasm_bindgen(js_namespace = window, js_name = Toastify)]
    pub fn toasts(options: &JsValue) -> Toastify;
    #[wasm_bindgen(method, js_name = showToast)]
    pub fn show_toast(this: &Toastify);
}
