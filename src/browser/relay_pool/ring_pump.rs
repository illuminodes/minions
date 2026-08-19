//! Wakes the application loop to read the outbound ring.
//!
//! This is the application-side twin of
//! [`RingPoller`](super::ring_poller::RingPoller), and it exists for the same
//! reason: a `postMessage` is an event that wakes its reader, but a ring is
//! passive memory, so a note written into it wakes nobody.
//!
//! # The bug this prevents
//!
//! The driver loop waits on `select!` over the command channel and the bridge.
//! Once the rings go live the worker stops sending bridge frames, because the
//! notes now travel through the rings. So the bridge arm stops firing, the loop
//! stops turning, and it never reads the ring it just adopted. The transport
//! reports success and delivers nothing.
//!
//! This pump gives the loop a third `select!` arm that fires on its own, so the
//! ring is drained whether or not any bridge traffic arrives.
//!
//! # Why an animation frame and not a timer
//!
//! Notes are only observable once they are painted. A frame callback runs at
//! the display's own rate, so it drains as often as the result can be seen and
//! stops entirely in a background tab. A 16ms timer keeps firing there and
//! burns battery to compute frames nobody sees.

use std::cell::RefCell;

use futures::channel::mpsc;
use wasm_bindgen::prelude::*;

/// The frame callback, held alive and shared with itself so each frame can
/// request the next one.
type FrameCallback = std::rc::Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// A repeating wake-up for the application loop.
///
/// The handle is shared with the frame callback, not copied out of it: each
/// frame requests the next one and stores a NEW handle. Cancelling the handle
/// captured at start would cancel a frame that already ran and leave the live
/// one going.
pub struct RingPump {
    handle: std::rc::Rc<RefCell<Option<i32>>>,
    state: FrameCallback,
}

impl RingPump {
    /// Start pumping, sending one tick per animation frame.
    ///
    /// Returns `None` off the main thread, or when no frame can be requested;
    /// the loop then still serves the bridge, so a non-isolated page is
    /// unaffected.
    #[must_use]
    pub fn start(tick_tx: mpsc::UnboundedSender<()>) -> Option<Self> {
        let window = web_sys::window()?;
        let state: FrameCallback = std::rc::Rc::new(RefCell::new(None));
        let handle = std::rc::Rc::new(RefCell::new(None));

        let frame = {
            let state = state.clone();
            let handle = handle.clone();
            let window = window.clone();
            Closure::wrap(Box::new(move |_ts: f64| {
                if tick_tx.unbounded_send(()).is_err() {
                    state.borrow_mut().take();
                    return;
                }
                let next = state.borrow().as_ref().and_then(|cb| {
                    window
                        .request_animation_frame(cb.as_ref().unchecked_ref())
                        .ok()
                });
                *handle.borrow_mut() = next;
            }) as Box<dyn FnMut(f64)>)
        };

        let first = window
            .request_animation_frame(frame.as_ref().unchecked_ref())
            .ok()?;
        *handle.borrow_mut() = Some(first);
        *state.borrow_mut() = Some(frame);

        Some(Self { handle, state })
    }
}

impl Drop for RingPump {
    fn drop(&mut self) {
        self.state.borrow_mut().take();
        let Some(handle) = self.handle.borrow_mut().take() else {
            return;
        };
        if let Some(window) = web_sys::window() {
            let _ = window.cancel_animation_frame(handle);
        }
    }
}
