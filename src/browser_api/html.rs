use web_sys::wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{HtmlFormElement, HtmlInputElement, HtmlSelectElement, SubmitEvent};

pub struct HtmlDocument {
    pub window: web_sys::Window,
    pub document: web_sys::Document,
}
impl HtmlDocument {
    pub fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window available"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document available"))?;
        Ok(Self { window, document })
    }
    pub fn find_element_by_id<T>(&self, id: &str) -> Result<T, JsValue>
    where
        T: JsCast,
    {
        self.document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str("Element not found"))?
            .dyn_into::<T>()
            .map_err(|_| JsValue::from_str("Failed to cast element"))
    }
    pub fn query_selector<T>(&self, selector: &str) -> Result<T, JsValue>
    where
        T: JsCast,
    {
        self.document
            .query_selector(selector)?
            .ok_or_else(|| JsValue::from_str("Elements not found"))?
            .dyn_into::<T>()
            .map_err(|_| JsValue::from_str("Failed to cast element"))
    }
    pub fn edit_document_title(self, title: &str) {
        self.document.set_title(title);

        let closure: web_sys::js_sys::Function = Closure::<dyn FnMut()>::new(move || {
            self.document.set_title("Portal SALUD");
        })
        .into_js_value()
        .into();
        let _ = self
            .window
            .add_event_listener_with_callback("focus", &closure);
    }
}

pub struct HtmlForm {
    form: HtmlFormElement,
}
impl HtmlForm {
    pub fn new(submit_event: &SubmitEvent) -> Result<Self, JsValue> {
        let form = submit_event.target();
        if form.is_none() {
            return Err(JsValue::from_str("Form not found"));
        }
        let form = form
            .map(JsCast::unchecked_into::<web_sys::HtmlFormElement>)
            .ok_or_else(|| JsValue::from_str("Failed to cast form"))?;
        Ok(Self { form })
    }
    pub fn input<T>(&self, name: &str) -> Result<T, JsValue>
    where
        T: JsCast,
    {
        let input = self.form.get_with_name(name);
        if input.is_none() {
            return Err(JsValue::from_str("Input not found"));
        }
        input
            .map(JsCast::unchecked_into::<T>)
            .ok_or_else(|| JsValue::from_str("Failed to cast input"))
    }
    pub fn input_value(&self, name: &str) -> Result<String, JsValue> {
        Ok(self.input::<HtmlInputElement>(name)?.value())
    }
    pub fn select_value(&self, name: &str) -> Result<String, JsValue> {
        Ok(self.input::<HtmlSelectElement>(name)?.value())
    }
    pub fn textarea_value(&self, name: &str) -> Result<String, JsValue> {
        Ok(self.input::<web_sys::HtmlTextAreaElement>(name)?.value())
    }
}
