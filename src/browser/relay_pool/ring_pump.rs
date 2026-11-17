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
//!
//! # Why a slow timer as well
//!
//! Stopping entirely is not safe either. The worker's sockets keep receiving
//! while the tab is hidden, so with nobody draining, the ring fills and every
//! further frame is held on the worker's heap for as long as the tab stays in
//! the background. A second, slow tick keeps the ring moving there. While the
//! tab is visible it adds one redundant drain per second.

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
    /// The slow tick that still fires while the tab is hidden.
    background: Option<(i32, Closure<dyn FnMut()>)>,
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
        let background_tx = tick_tx.clone();
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
        // Started last, so an early return above cannot leave a timer running
        // with nothing left to clear it.
        let background = Self::start_background(&window, background_tx);

        Some(Self {
            handle,
            state,
            background,
        })
    }

    /// How often to tick when no animation frame arrives.
    const BACKGROUND_INTERVAL_MS: i32 = 1000;

    /// Start the slow tick. `None` when the timer cannot start, which leaves
    /// the animation-frame pump working as before.
    fn start_background(
        window: &web_sys::Window,
        tick_tx: mpsc::UnboundedSender<()>,
    ) -> Option<(i32, Closure<dyn FnMut()>)> {
        let tick = Closure::wrap(Box::new(move || {
            let _ = tick_tx.unbounded_send(());
        }) as Box<dyn FnMut()>);
        let handle = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                tick.as_ref().unchecked_ref(),
                Self::BACKGROUND_INTERVAL_MS,
            )
            .ok()?;
        Some((handle, tick))
    }

    /// How many ring batches one turn of the driver takes while hidden.
    const HIDDEN_BATCHES: usize = 64;

    /// How many ring batches the driver should take this turn.
    ///
    /// One bounded batch per turn keeps a burst from becoming a long frame.
    /// A hidden page paints no frames and turns the driver only on the slow
    /// tick, where a single batch would let the worker's backlog grow without
    /// limit, so there it takes many.
    #[must_use]
    pub fn batches_per_turn() -> usize {
        if Self::page_hidden() {
            Self::HIDDEN_BATCHES
        } else {
            1
        }
    }

    /// Whether the page is in a background tab, where no frame is painted.
    #[must_use]
    pub fn page_hidden() -> bool {
        web_sys::window()
            .and_then(|window| window.document())
            .is_some_and(|document| document.hidden())
    }
}

impl Drop for RingPump {
    fn drop(&mut self) {
        if let (Some((timer, _)), Some(window)) = (self.background.take(), web_sys::window()) {
            window.clear_interval_with_handle(timer);
        }
        self.state.borrow_mut().take();
        let Some(handle) = self.handle.borrow_mut().take() else {
            return;
        };
        if let Some(window) = web_sys::window() {
            let _ = window.cancel_animation_frame(handle);
        }
    }
}
