pub mod crypto;
pub mod geolocation;
pub mod indexed_db;
pub mod html;
pub mod service_worker;

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_get_geolocation() {
        let position = geolocation::GeolocationPosition::locate().await;
        assert!(position.is_ok());
    }
}
