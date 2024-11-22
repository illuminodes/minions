mod crypto;
mod geolocation;
mod html;
mod indexed_db;
mod service_worker;

pub use crypto::BrowserCrypto;
pub use geolocation::{GeolocationPosition, GeolocationCoordinates};
pub use html::{HtmlDocument, HtmlForm};
pub use indexed_db::*;
pub use service_worker::AppServiceWorker;
pub fn clipboard_copy(message: &str) {
    use wasm_bindgen_futures::{spawn_local, JsFuture};
    use web_sys::window;
    let promise = window().unwrap().navigator().clipboard().write_text(message);
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}
