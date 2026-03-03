use web_sys::wasm_bindgen::JsValue;

pub struct AppServiceWorker {
    sw: web_sys::ServiceWorkerContainer,
}
impl AppServiceWorker {
    pub fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or(JsValue::from_str("No window"))?;
        let sw = window.navigator().service_worker();
        Ok(Self { sw })
    }
    pub async fn install(&self, file_path: &str) -> Result<(), JsValue> {
        // `register` returns Result<Promise, JsValue>. We unwrap it and wait for the promise.
        let register_promise = self.sw.register(file_path)?;
        wasm_bindgen_futures::JsFuture::from(register_promise).await?;
        Ok(())
    }
}
