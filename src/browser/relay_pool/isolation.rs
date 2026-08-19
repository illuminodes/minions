//! Runtime detection of whether the shared-memory note transport is usable.
//!
//! A `SharedArrayBuffer` exists only on a **cross-origin isolated** page. That
//! is a property of the HTTP response headers the consumer's server sends:
//!
//! ```text
//! Cross-Origin-Opener-Policy: same-origin
//! Cross-Origin-Embedder-Policy: require-corp
//! ```
//!
//! A library cannot set those headers, and they are not free for the consumer:
//! `require-corp` blocks cross-origin subresources that do not opt in, which
//! breaks many third-party embeds. So the pool must ASK at runtime rather than
//! assume, and fall back to the JSON bridge when the answer is no.

use wasm_bindgen::JsValue;
use web_sys::js_sys;

/// Whether this page can use the shared-memory transport.
pub struct Isolation;

impl Isolation {
    /// True when the page is cross-origin isolated AND `SharedArrayBuffer` is
    /// actually constructible.
    ///
    /// Both checks matter. `crossOriginIsolated` can be true while the
    /// constructor is missing on an old engine, and some environments expose
    /// the constructor while refusing to allocate. Probing both keeps the
    /// decision to one place instead of scattering `catch` blocks through the
    /// transport.
    #[must_use]
    pub fn is_available() -> bool {
        Self::cross_origin_isolated() && Self::constructor_present()
    }

    fn cross_origin_isolated() -> bool {
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crossOriginIsolated"))
            .is_ok_and(|v| v.is_truthy())
    }

    fn constructor_present() -> bool {
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("SharedArrayBuffer"))
            .is_ok_and(|v| !v.is_undefined() && !v.is_null())
    }

    /// A one-line explanation for logs when the transport is unavailable.
    #[must_use]
    pub fn explain() -> &'static str {
        if !Self::constructor_present() {
            "SharedArrayBuffer is not defined in this environment"
        } else if !Self::cross_origin_isolated() {
            "page is not cross-origin isolated (needs COOP: same-origin + COEP: require-corp)"
        } else {
            "shared-memory transport is available"
        }
    }
}
