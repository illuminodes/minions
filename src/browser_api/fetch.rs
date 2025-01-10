use web_sys::wasm_bindgen::JsCast;

pub struct BrowserFetch {}
impl BrowserFetch {
    pub async fn request<T>(request: &web_sys::Request) -> Result<T, web_sys::wasm_bindgen::JsValue>
    where
        T: TryFrom<web_sys::wasm_bindgen::JsValue>,
        web_sys::wasm_bindgen::JsValue: From<<T as TryFrom<web_sys::wasm_bindgen::JsValue>>::Error>,
    {
        let window = web_sys::window().ok_or(web_sys::wasm_bindgen::JsValue::from_str(
            "No window available",
        ))?;
        let response =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(request)).await?;
        let response: web_sys::Response = response.dyn_into()?;
        let response_json = wasm_bindgen_futures::JsFuture::from(response.json()?).await?;
        let response_json: T = response_json.try_into()?;
        Ok(response_json)
    }
    pub async fn request_with_init<T>(
        request: &web_sys::Request,
        init: &web_sys::RequestInit,
    ) -> Result<T, web_sys::wasm_bindgen::JsValue>
    where
        T: TryFrom<web_sys::wasm_bindgen::JsValue>,
        web_sys::wasm_bindgen::JsValue: From<<T as TryFrom<web_sys::wasm_bindgen::JsValue>>::Error>,
    {
        let window = web_sys::window().ok_or(web_sys::wasm_bindgen::JsValue::from_str(
            "No window available",
        ))?;
        let response =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_request_and_init(request, init))
                .await?;
        let response: web_sys::Response = response.dyn_into()?;
        let response_json = wasm_bindgen_futures::JsFuture::from(response.json()?).await?;
        let response_json: T = response_json.try_into()?;
        Ok(response_json)
    }
}
