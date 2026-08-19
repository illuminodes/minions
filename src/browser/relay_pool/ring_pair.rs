//! Two byte rings, one per direction, and the handle that carries them to the
//! worker.
//!
//! A [`SabRing`] is single-producer/single-consumer, so it moves bytes ONE way.
//! Full duplex needs two: the application writes into `to_worker` and reads
//! from `to_app`, and the worker does the opposite.
//!
//! Getting that backwards would be a silent deadlock, not a compile error, so
//! neither side picks its rings by hand. Each names its [`Role`] once and
//! [`RingPair::endpoint`] wires the directions.

use std::rc::Rc;

use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Object, Reflect};

use super::endpoint::Endpoint;
use super::sab_ring::SabRing;

/// Property names on the handle object posted to the worker.
const TO_WORKER: &str = "toWorker";
const TO_APP: &str = "toApp";

/// Which side of the pair a thread is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// The UI thread: sends commands, receives notes and events.
    App,
    /// The relay worker: receives commands, sends notes and events.
    Worker,
}

/// Both directions of the shared transport.
pub struct RingPair {
    to_worker: Rc<SabRing>,
    to_app: Rc<SabRing>,
}

impl RingPair {
    /// Allocate both rings.
    ///
    /// Call this ONLY when [`super::isolation::Isolation::is_available`] is
    /// true; see [`SabRing::create`].
    #[must_use]
    pub fn create() -> Self {
        Self {
            to_worker: Rc::new(SabRing::create()),
            to_app: Rc::new(SabRing::create()),
        }
    }

    /// A plain JS object holding both buffers, to `postMessage` to the worker.
    ///
    /// `SharedArrayBuffer`s are shared by reference, so this is a handle, not a
    /// copy.
    #[must_use]
    pub fn handles(&self) -> JsValue {
        let handles = Object::new();
        let _ = Reflect::set(&handles, &TO_WORKER.into(), &self.to_worker.buffer());
        let _ = Reflect::set(&handles, &TO_APP.into(), &self.to_app.buffer());
        handles.into()
    }

    /// Rebuild the pair on the worker from [`Self::handles`].
    ///
    /// # Errors
    /// Returns `Err` when the value is not the expected object, or either
    /// property is not a `SharedArrayBuffer`.
    pub fn attach(handles: &JsValue) -> Result<Self, JsValue> {
        let to_worker = SabRing::attach(Reflect::get(handles, &TO_WORKER.into())?)?;
        let to_app = SabRing::attach(Reflect::get(handles, &TO_APP.into())?)?;
        Ok(Self {
            to_worker: Rc::new(to_worker),
            to_app: Rc::new(to_app),
        })
    }

    /// This thread's send/receive pair, with the directions bound to `role`.
    #[must_use]
    pub fn endpoint(&self, role: Role) -> Endpoint {
        match role {
            Role::App => Endpoint::new(Rc::clone(&self.to_worker), Rc::clone(&self.to_app)),
            Role::Worker => Endpoint::new(Rc::clone(&self.to_app), Rc::clone(&self.to_worker)),
        }
    }
}
