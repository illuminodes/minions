//! The worker-to-application ring handshake.
//!
//! # Why the worker allocates the rings
//!
//! `yew-agent` keeps its `web_sys::Worker` private: `ReactorBridge` exposes
//! only `send`/`fork`, and `ReactorSpawner` only `spawn`. The application
//! therefore has NO object to `postMessage` a `SharedArrayBuffer` on, and the
//! handles cannot travel as a `RelayCommand` because the codec is JSON.
//!
//! The worker has no such problem: its global scope is always reachable. So the
//! worker allocates both rings and posts the handles to the application, and
//! the direction of the handshake is the opposite of the data flow it sets up.
//!
//! # Why this lives in the codec
//!
//! `yew-agent` installs its own `onmessage` on the worker at spawn time, and
//! the application never gets the object needed to add a second listener. The
//! only place application-side code sees a raw `JsValue` from the worker is
//! [`Codec::decode`](yew_agent::Codec). So the handshake frame is claimed
//! there, before `yew-agent` tries to read it as one of its own messages.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Object, Reflect};

thread_local! {
    static CLAIMED: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

/// Claims and stores the ring handles the worker posts at startup.
pub struct RingHandshake;

impl RingHandshake {
    /// Marks the one message that is ours and not `yew-agent`'s.
    ///
    /// Regular bridge traffic is a JSON **string**; a handshake is an object
    /// carrying this property. The two can never be confused.
    const MARKER: &'static str = "__nostrMinionsRings";

    /// Wrap the ring handles as the frame the application will recognise.
    #[must_use]
    pub fn frame(handles: &JsValue) -> JsValue {
        let frame = Object::new();
        let _ = Reflect::set(&frame, &Self::MARKER.into(), handles);
        frame.into()
    }

    /// Take the handles out of a frame, if it is a handshake.
    pub fn unwrap_frame(value: &JsValue) -> Option<JsValue> {
        if value.is_string() {
            return None;
        }
        Reflect::get(value, &Self::MARKER.into())
            .ok()
            .filter(|handles| !handles.is_undefined() && !handles.is_null())
    }

    /// Store a handshake frame and report whether it was one.
    ///
    /// The caller must not process a frame for which this returns `true`.
    pub fn claim(value: &JsValue) -> bool {
        let Some(handles) = Self::unwrap_frame(value) else {
            return false;
        };
        CLAIMED.with(|slot| slot.borrow_mut().replace(handles));
        true
    }

    /// The handles the worker sent, if they have arrived.
    #[must_use]
    pub fn handles() -> Option<JsValue> {
        CLAIMED.with(|slot| slot.borrow().clone())
    }

    /// What a claimed handshake decodes to, so `yew-agent` sees a valid message.
    ///
    /// `Codec::decode` must return the caller's type, and the application
    /// decodes `yew_agent`'s private `FromWorker` enum. `WorkerLoaded` is the
    /// only variant expressible without its private `HandlerId`, and it is
    /// externally tagged, so this is its exact JSON.
    ///
    /// `WorkerLoaded` is NOT inert: it flushes the bridge's pending queue to
    /// the worker. That is safe only because the worker posts the handshake
    /// AFTER it registers the reactor, so a listener is already installed and
    /// the flushed messages cannot be lost. The real `WorkerLoaded` that
    /// follows then finds the queue empty and does nothing, which is why the
    /// duplicate is harmless.
    pub const DECODES_AS: &'static str = "\"WorkerLoaded\"";
}
