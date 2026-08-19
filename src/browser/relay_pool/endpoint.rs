//! One thread's view of the shared transport: a ring it sends on and a ring it
//! receives from.
//!
//! Both threads use this same type. Which physical ring is which is decided by
//! [`super::ring_pair::RingPair::endpoint`], so nothing here needs to know
//! whether it runs on the UI thread or in the worker.
//!
//! # Delivery
//!
//! Sending never drops and never blocks. A full ring makes
//! [`frame_queue::FrameQueue`] hold the frame in this thread's own heap and
//! retry it, in order, ahead of anything newer. Receiving never blocks either:
//! [`Self::recv`] takes what is there and returns.

use std::cell::RefCell;
use std::rc::Rc;

use crate::transport::FrameQueue;

use super::sab_ring::SabRing;

/// A send ring plus a receive ring, with the spill buffer for the send side.
pub struct Endpoint {
    send_ring: Rc<SabRing>,
    recv_ring: Rc<SabRing>,
    pending: RefCell<FrameQueue>,
}

impl Endpoint {
    pub(crate) const fn new(send_ring: Rc<SabRing>, recv_ring: Rc<SabRing>) -> Self {
        Self {
            send_ring,
            recv_ring,
            pending: RefCell::new(FrameQueue::new()),
        }
    }

    /// Queue one frame for the peer. Never drops it, never blocks.
    pub fn send(&self, frame: Vec<u8>) {
        self.pending.borrow_mut().send(&*self.send_ring, frame);
    }

    /// Retry frames the ring refused earlier.
    ///
    /// Call this when the peer may have made room — on the drain tick, or after
    /// the peer signals progress. Returns how many frames moved.
    pub fn flush(&self) -> usize {
        self.pending.borrow_mut().flush(&*self.send_ring)
    }

    /// Take up to `max` frames from the peer. Returns fewer when the ring runs
    /// dry, and never blocks waiting for more.
    ///
    /// `max` bounds the work done in one turn so a flood cannot monopolise the
    /// thread; the rest waits for the next call.
    pub fn recv(&self, max: usize) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        for _ in 0..max {
            let Some(frame) = self.recv_ring.pop() else {
                break;
            };
            frames.push(frame);
        }
        frames
    }

    /// Frames still held in this thread's heap because the ring was full.
    pub fn backlog(&self) -> usize {
        self.pending.borrow().backlog()
    }

    /// The largest backlog ever reached. Non-zero means the peer fell behind;
    /// it never means data was lost.
    #[allow(dead_code, reason = "reporting it needs a metrics frame; see worker_sink")]
    pub fn peak_backlog(&self) -> usize {
        self.pending.borrow().peak()
    }

    /// Announce that this thread is draining, so the peer keeps sending.
    pub fn open(&self) {
        self.recv_ring.set_consumer_alive(true);
    }

    /// Announce that this thread has stopped draining.
    ///
    /// The peer stops encoding once it reads this, instead of growing a backlog
    /// for a reader that will never return.
    pub fn close(&self) {
        self.recv_ring.set_consumer_alive(false);
    }

    /// Whether the peer is still draining what this thread sends.
    ///
    /// Kept because [`Self::close`] writes this flag, so the pair is what makes
    /// the signal meaningful: a writer with no way to read it could never act
    /// on a reader that went away.
    #[allow(dead_code, reason = "the read half of the close signal; no caller yet")]
    pub fn peer_listening(&self) -> bool {
        self.send_ring.consumer_alive()
    }
}
