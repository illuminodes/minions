use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub type BeforeInstallPromptEvent;
    #[wasm_bindgen(method)]
    pub fn prompt(this: &BeforeInstallPromptEvent) -> web_sys::js_sys::Promise;
    #[wasm_bindgen(method, js_name = "preventDefault")]
    pub fn prevent_default(this: &BeforeInstallPromptEvent);
}


