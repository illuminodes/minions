//! Wakes the worker loop to read the inbound ring.
//!
//! The bridge wakes its reader by itself: a `postMessage` is an event. A ring
//! is passive memory, so a frame written into it wakes nobody. Something must
//! ask.
//!
//! This poller is what makes the two transports interchangeable to the reactor
//! loop: with it, a ring command reaches [`RelaySet`](super::relay_set::RelaySet)
//! by the same path a bridge command does.
//!
//! # Why a timer and not `Atomics::wait`
//!
//! The thread that reads this ring also runs the `WebSocket` callbacks. Parking
//! it on `Atomics::wait` would stall the sockets, and relays drop a connection
//! whose consumer stops reading. `Atomics::wait_async` avoids the park and is
//! the eventual replacement; a timer is correct in the meantime because no
//! frame is ever lost by being read late.

use std::cell::RefCell;

use futures::channel::mpsc;
use wasm_bindgen::prelude::*;

use super::worker::Internal;

/// A repeating wake-up for the worker loop.
pub struct RingPoller {
    handle: RefCell<Option<i32>>,
    _tick: Closure<dyn FnMut()>,
}

impl RingPoller {
    /// How often to look at the ring.
    ///
    /// Commands are user actions (subscribe, publish), which are rare and not
    /// latency-critical at this scale. Notes travel the OTHER way and are never
    /// delayed by this interval.
    const INTERVAL_MS: i32 = 16;

    /// Start polling, sending [`Internal::Poll`] on every tick.
    ///
    /// Returns `None` off the worker thread, or when the timer cannot start; the
    /// loop then still serves the bridge, so nothing breaks.
    #[must_use]
    pub fn start(bus_tx: mpsc::UnboundedSender<Internal>) -> Option<Self> {
        let tick = Closure::wrap(Box::new(move || {
            let _ = bus_tx.unbounded_send(Internal::Poll);
        }) as Box<dyn FnMut()>);

        let scope = web_sys::js_sys::global()
            .dyn_into::<web_sys::DedicatedWorkerGlobalScope>()
            .ok()?;
        let handle = scope
            .set_interval_with_callback_and_timeout_and_arguments_0(
                tick.as_ref().unchecked_ref(),
                Self::INTERVAL_MS,
            )
            .ok()?;

        Some(Self {
            handle: RefCell::new(Some(handle)),
            _tick: tick,
        })
    }
}

impl Drop for RingPoller {
    fn drop(&mut self) {
        let Some(handle) = self.handle.borrow_mut().take() else {
            return;
        };
        if let Ok(scope) =
            web_sys::js_sys::global().dyn_into::<web_sys::DedicatedWorkerGlobalScope>()
        {
            scope.clear_interval_with_handle(handle);
        }
    }
}
