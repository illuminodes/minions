//! Reactor-path drain: move notes from the bridge's `pending` queue into the
//! render buffer, bounded per animation frame.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use nostro2::NostrNote;
use yew::prelude::*;

use crate::raf::RafLoop;
use crate::{SharedMetrics, DRAIN_PER_FRAME, LIMIT};

/// Owns both ends of the reactor handoff, so the queue the bridge fills and the
/// queue the frame drains cannot diverge.
#[derive(Clone)]
pub struct Drain {
    notes: Rc<RefCell<VecDeque<NostrNote>>>,
    pending: Rc<RefCell<VecDeque<NostrNote>>>,
    metrics: SharedMetrics,
}

impl Drain {
    #[must_use]
    pub fn new(
        notes: Rc<RefCell<VecDeque<NostrNote>>>,
        pending: Rc<RefCell<VecDeque<NostrNote>>>,
        metrics: SharedMetrics,
    ) -> Self {
        Self {
            notes,
            pending,
            metrics,
        }
    }

    fn step(&self) -> usize {
        self.metrics.borrow_mut().drain_frames += 1;
        let mut pend = self.pending.borrow_mut();
        if pend.is_empty() {
            return 0;
        }
        let take = pend.len().min(DRAIN_PER_FRAME);
        let mut buf = self.notes.borrow_mut();
        for _ in 0..take {
            if let Some(note) = pend.pop_front() {
                buf.push_front(note);
            }
        }
        buf.truncate(LIMIT as usize);
        drop(buf);
        drop(pend);
        self.metrics.borrow_mut().throughput.render(take as u64);
        take
    }
}

#[hook]
pub fn use_drain(drain: Drain) {
    let force = use_force_update();
    use_effect_with((), move |()| {
        let raf = RafLoop::start(move |_ts| {
            if drain.step() > 0 {
                force.force_update();
            }
        });
        move || drop(raf)
    });
}
