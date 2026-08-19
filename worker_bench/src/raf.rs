//! A cancellable `requestAnimationFrame` chain.
//!
//! The naive teardown — drop the `Closure` and let the in-flight frame find an
//! empty cell — is unsound. The browser still holds the already-scheduled
//! callback, and invoking a dropped `Closure` makes wasm-bindgen throw
//! "closure invoked recursively or after being dropped". That exception
//! escapes through yew's `run_scheduler`, which aborts the rest of the effect
//! queue: every effect that was still pending never mounts.
//!
//! `RafLoop` keeps the frame handle and calls `cancelAnimationFrame` before it
//! drops the closure, so no dead callback is ever invoked.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Default)]
struct RafState {
    handle: Option<i32>,
    closure: Option<Closure<dyn FnMut(f64)>>,
    stopped: bool,
}

pub struct RafLoop {
    state: Rc<RefCell<RafState>>,
}

impl RafLoop {
    /// Start a chain that calls `tick` once per animation frame.
    #[must_use]
    pub fn start<F>(mut tick: F) -> Self
    where
        F: FnMut(f64) + 'static,
    {
        let state: Rc<RefCell<RafState>> = Rc::new(RefCell::new(RafState::default()));
        let inner = state.clone();
        let closure = Closure::wrap(Box::new(move |ts: f64| {
            inner.borrow_mut().handle = None;
            if inner.borrow().stopped {
                return;
            }
            tick(ts);
            Self::schedule(&inner);
        }) as Box<dyn FnMut(f64)>);
        state.borrow_mut().closure = Some(closure);
        Self::schedule(&state);
        Self { state }
    }

    fn schedule(state: &Rc<RefCell<RafState>>) {
        let mut guard = state.borrow_mut();
        if guard.stopped {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(closure) = guard.closure.as_ref() else {
            return;
        };
        guard.handle = window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .ok();
    }

    pub fn stop(&self) {
        let mut guard = self.state.borrow_mut();
        guard.stopped = true;
        if let (Some(handle), Some(window)) = (guard.handle.take(), web_sys::window()) {
            let _ = window.cancel_animation_frame(handle);
        }
        guard.closure = None;
    }
}

impl Drop for RafLoop {
    fn drop(&mut self) {
        self.stop();
    }
}
