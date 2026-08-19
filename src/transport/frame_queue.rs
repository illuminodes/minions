//! Loss-free ordered handoff to a bounded, non-blocking sink.
//!
//! # Why this exists
//!
//! The shared ring is bounded. A producer that meets a full ring has three
//! options, and two of them are wrong for a library:
//!
//! - **Drop the frame.** Loses notes. A relay does not resend on its own, so
//!   the consumer silently sees an incomplete stream.
//! - **Block until space frees.** Deadlocks. The producer thread also runs the
//!   WebSocket callbacks, so parking it stops the sockets. If the consumer is
//!   throttled (backgrounded tab) the park never ends and the relay drops the
//!   connection.
//! - **Spill to the producer's own heap.** Never loses, never parks. This
//!   module.
//!
//! The cost of spilling is memory when the consumer falls behind. That is the
//! same trade the relay worker's internal bus already makes with an unbounded
//! channel, so the policy is consistent across the crate.
//!
//! # Order
//!
//! Order is the property that makes this subtle. Once one frame spills, EVERY
//! later frame must spill too, even when the sink has room again. A frame that
//! goes straight to the sink while a backlog exists would overtake the backlog
//! and reorder the stream. [`FrameQueue::send`] enforces this.

use std::collections::VecDeque;

/// A bounded sink that refuses a frame instead of blocking.
///
/// Implemented by the shared ring, and by fakes in tests.
pub trait FrameSink {
    /// Try to accept one frame. Return `false` when full, having consumed
    /// nothing. Never block.
    fn offer(&self, frame: &[u8]) -> bool;
}

/// Ordered, loss-free front end to a [`FrameSink`].
///
/// Frames go straight through while the sink has room. When the sink fills,
/// frames queue here and drain in order on a later [`send`](Self::send) or
/// [`flush`](Self::flush). Nothing is ever dropped.
#[derive(Debug, Default)]
pub struct FrameQueue {
    backlog: VecDeque<Vec<u8>>,
    peak: usize,
}

impl FrameQueue {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            backlog: VecDeque::new(),
            peak: 0,
        }
    }

    /// Hand one frame to the sink, or queue it when the sink is full.
    ///
    /// Drains the backlog first, so frames reach the consumer in submission
    /// order.
    pub fn send(&mut self, sink: &impl FrameSink, frame: Vec<u8>) {
        self.flush(sink);
        if self.backlog.is_empty() && sink.offer(&frame) {
            return;
        }
        self.backlog.push_back(frame);
        self.peak = self.peak.max(self.backlog.len());
    }

    /// Push as much of the backlog into the sink as it will take.
    ///
    /// Returns the number of frames that moved. Stops at the first refusal so
    /// order holds.
    pub fn flush(&mut self, sink: &impl FrameSink) -> usize {
        let mut moved = 0;
        while let Some(frame) = self.backlog.front() {
            if !sink.offer(frame) {
                break;
            }
            self.backlog.pop_front();
            moved += 1;
        }
        moved
    }

    /// Frames waiting in the producer's heap.
    #[must_use]
    pub fn backlog(&self) -> usize {
        self.backlog.len()
    }

    /// Largest backlog seen. Surfaces consumer lag that never lost data.
    #[must_use]
    pub const fn peak(&self) -> usize {
        self.peak
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.backlog.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameQueue, FrameSink};
    use std::cell::RefCell;

    /// A sink with a settable capacity, recording what it accepted.
    struct FakeSink {
        accepted: RefCell<Vec<Vec<u8>>>,
        capacity: RefCell<usize>,
    }

    impl FakeSink {
        fn with_capacity(capacity: usize) -> Self {
            Self {
                accepted: RefCell::new(Vec::new()),
                capacity: RefCell::new(capacity),
            }
        }

        fn set_capacity(&self, capacity: usize) {
            *self.capacity.borrow_mut() = capacity;
        }

        fn drain(&self) -> Vec<Vec<u8>> {
            self.accepted.borrow_mut().drain(..).collect()
        }

        fn accepted_count(&self) -> usize {
            self.accepted.borrow().len()
        }
    }

    impl FrameSink for FakeSink {
        fn offer(&self, frame: &[u8]) -> bool {
            let mut capacity = self.capacity.borrow_mut();
            if *capacity == 0 {
                return false;
            }
            *capacity -= 1;
            self.accepted.borrow_mut().push(frame.to_vec());
            true
        }
    }

    #[test]
    fn roomy_sink_takes_frames_directly() {
        let sink = FakeSink::with_capacity(8);
        let mut queue = FrameQueue::new();
        queue.send(&sink, b"a".to_vec());
        queue.send(&sink, b"b".to_vec());
        assert_eq!(queue.backlog(), 0);
        assert_eq!(sink.drain(), vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn full_sink_spills_instead_of_dropping() {
        let sink = FakeSink::with_capacity(1);
        let mut queue = FrameQueue::new();
        queue.send(&sink, b"a".to_vec());
        queue.send(&sink, b"b".to_vec());
        queue.send(&sink, b"c".to_vec());
        assert_eq!(sink.accepted_count(), 1);
        assert_eq!(queue.backlog(), 2, "nothing may be lost when the sink fills");
    }

    #[test]
    fn backlog_drains_in_order_once_room_returns() {
        let sink = FakeSink::with_capacity(1);
        let mut queue = FrameQueue::new();
        for frame in [b"a", b"b", b"c"] {
            queue.send(&sink, frame.to_vec());
        }
        sink.set_capacity(8);
        assert_eq!(queue.flush(&sink), 2);
        assert!(queue.is_idle());
        assert_eq!(
            sink.drain(),
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
        );
    }

    #[test]
    fn fresh_frame_never_overtakes_a_backlog() {
        let sink = FakeSink::with_capacity(1);
        let mut queue = FrameQueue::new();
        queue.send(&sink, b"first".to_vec());
        queue.send(&sink, b"spilled".to_vec());
        sink.set_capacity(1);
        queue.send(&sink, b"fresh".to_vec());
        assert_eq!(
            sink.drain(),
            vec![b"first".to_vec(), b"spilled".to_vec()],
            "the spilled frame must claim the freed slot, not the fresh one"
        );
        assert_eq!(queue.backlog(), 1);
    }

    #[test]
    fn partial_flush_stops_at_the_first_refusal() {
        let sink = FakeSink::with_capacity(0);
        let mut queue = FrameQueue::new();
        for frame in [b"a", b"b", b"c"] {
            queue.send(&sink, frame.to_vec());
        }
        sink.set_capacity(2);
        assert_eq!(queue.flush(&sink), 2);
        assert_eq!(queue.backlog(), 1);
        assert_eq!(sink.drain(), vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn every_frame_survives_a_long_stall() {
        let sink = FakeSink::with_capacity(0);
        let mut queue = FrameQueue::new();
        let sent: Vec<Vec<u8>> = (0..500u32).map(|i| i.to_le_bytes().to_vec()).collect();
        for frame in &sent {
            queue.send(&sink, frame.clone());
        }
        assert_eq!(queue.backlog(), 500);
        sink.set_capacity(usize::MAX);
        queue.flush(&sink);
        assert!(queue.is_idle());
        assert_eq!(sink.drain(), sent, "a total stall must not lose or reorder");
    }

    #[test]
    fn peak_records_worst_lag() {
        let sink = FakeSink::with_capacity(0);
        let mut queue = FrameQueue::new();
        for frame in [b"a", b"b", b"c"] {
            queue.send(&sink, frame.to_vec());
        }
        sink.set_capacity(8);
        queue.flush(&sink);
        assert_eq!(queue.backlog(), 0);
        assert_eq!(queue.peak(), 3, "peak must outlive the backlog it measured");
    }

    #[test]
    fn empty_frame_is_a_frame_like_any_other() {
        let sink = FakeSink::with_capacity(0);
        let mut queue = FrameQueue::new();
        queue.send(&sink, Vec::new());
        assert_eq!(queue.backlog(), 1);
        sink.set_capacity(1);
        assert_eq!(queue.flush(&sink), 1);
        assert_eq!(sink.drain(), vec![Vec::<u8>::new()]);
    }
}
