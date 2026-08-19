//! A single-producer/single-consumer byte ring inside a `SharedArrayBuffer`.
//!
//! The relay worker writes matched notes here instead of posting one JSON
//! message per note across the `yew-agent` bridge. The UI thread drains the
//! ring on an animation frame and decodes flat frames, so it never runs a JSON
//! parse per note.
//!
//! # Layout
//!
//! - bytes `0..16` — `i32` header slots, addressed by [`Slot`]
//! - bytes `16..16+cap` — the data region, a power-of-two byte ring
//!
//! A frame is a 4-byte little-endian length followed by that many payload
//! bytes, padded so every frame starts 4-byte aligned. A length of `-1` is a
//! skip marker meaning "no more frames before the end of the region, resume at
//! offset 0"; it keeps every frame contiguous, so each one moves in a single
//! typed-array copy.
//!
//! Only the producer advances `Tail` and only the consumer advances `Head`.
//! The atomic head/tail stores order the non-atomic payload reads and writes
//! around them.
//!
//! # Casts
//!
//! Byte counts are `u32` and header slots are `i32`, so [`SabRing::as_word`]
//! converts between them. The conversion is deliberately allowed to wrap:
//! cursors are free-running, compared only for equality, and masked into the
//! region by [`RingIndex`].
//!
//! # Neither end ever blocks
//!
//! [`SabRing::push`] refuses a frame when full and [`SabRing::pop`] returns
//! `None` when empty. Nothing here calls `Atomics::wait`.
//!
//! Blocking the consumer is obviously wrong — it is the UI thread, and not
//! blocking it is the point of this transport. Blocking the PRODUCER is just as
//! wrong, though less obviously: the worker thread that fills this ring also
//! runs the WebSocket callbacks. A parked producer stops reading sockets, and
//! if the consumer is throttled (a backgrounded tab gets no animation frames)
//! the park never ends and relays drop the stalled connections.
//!
//! So a refusal is not a loss and not a stall. [`frame_queue::FrameQueue`]
//! holds the refused frame in the producer's heap and retries it, in order,
//! before anything newer. This ring is the fast path; that queue is the
//! guarantee.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::js_sys::{Atomics, Int32Array, SharedArrayBuffer, Uint8Array};

use crate::transport::RingIndex;

const HEADER_BYTES: u32 = 16;

/// Length word meaning "no more frames before the end of the region".
const SKIP_MARKER: i32 = -1;

/// Data region size (8 MiB). Large enough that a burst of notes does not fill
/// the ring between two animation frames at any rate a relay produces.
const CAP_BYTES: u32 = 1 << 23;

/// Header slots, in `i32` units from the start of the buffer.
///
/// There is deliberately no drop counter: this transport never drops a frame,
/// so a counter for it could only ever read zero.
#[derive(Clone, Copy)]
enum Slot {
    Head = 0,
    Tail = 1,
    ConsumerAlive = 2,
}

impl Slot {
    const fn index(self) -> u32 {
        self as u32
    }
}

/// Both ends of the shared byte ring. The same type serves the producer (in the
/// worker) and the consumer (on the UI thread); the methods you call decide the
/// role.
pub struct SabRing {
    buffer: SharedArrayBuffer,
    header: Int32Array,
    words: Int32Array,
    data: Uint8Array,
    index: RingIndex,
}

#[allow(
    clippy::cast_possible_wrap,
    reason = "cursors are free-running i32s; wrapping is the intended arithmetic"
)]
impl SabRing {
    /// Allocate a new ring (UI thread).
    ///
    /// Call this ONLY after [`super::isolation::Isolation::is_available`]
    /// returns true. `SharedArrayBuffer` throws on a page which is not
    /// cross-origin isolated, and a JS exception crossing the wasm boundary
    /// aborts rather than unwinding, so the probe is the guard — not a
    /// `Result` here.
    #[must_use]
    pub fn create() -> Self {
        Self::view(SharedArrayBuffer::new(HEADER_BYTES + CAP_BYTES))
    }

    /// View a ring received from the other thread (worker side).
    ///
    /// # Errors
    /// Returns `Err` if `value` is not a `SharedArrayBuffer`.
    pub fn attach(value: JsValue) -> Result<Self, JsValue> {
        let buffer = value
            .dyn_into::<SharedArrayBuffer>()
            .map_err(|_| JsValue::from_str("sab ring: expected a SharedArrayBuffer"))?;
        Ok(Self::view(buffer))
    }

    fn view(buffer: SharedArrayBuffer) -> Self {
        let raw: &JsValue = buffer.as_ref();
        let header = Int32Array::new_with_byte_offset_and_length(raw, 0, 4);
        let words = Int32Array::new_with_byte_offset_and_length(raw, HEADER_BYTES, CAP_BYTES / 4);
        let data = Uint8Array::new_with_byte_offset_and_length(raw, HEADER_BYTES, CAP_BYTES);
        Self {
            buffer,
            header,
            words,
            data,
            index: RingIndex::new(CAP_BYTES),
        }
    }

    /// The underlying buffer, to hand to the worker in the handshake.
    #[must_use]
    pub fn buffer(&self) -> JsValue {
        self.buffer.clone().into()
    }

    fn load(&self, slot: Slot) -> i32 {
        Atomics::load(&self.header, slot.index()).unwrap_or(0)
    }

    fn store(&self, slot: Slot, value: i32) {
        let _ = Atomics::store(&self.header, slot.index(), value);
        let _ = Atomics::notify(&self.header, slot.index());
    }

    /// Mark the consumer present or gone.
    ///
    /// The producer reads this to know that draining has stopped for good: once
    /// the consumer is gone, a full ring will never free again, so the producer
    /// stops encoding instead of growing its backlog without limit.
    pub fn set_consumer_alive(&self, alive: bool) {
        self.store(Slot::ConsumerAlive, i32::from(alive));
    }

    #[must_use]
    #[allow(dead_code, reason = "the read half of the close signal; no caller yet")]
    pub fn consumer_alive(&self) -> bool {
        self.load(Slot::ConsumerAlive) != 0
    }

    /// Reinterpret a byte count as the `i32` the header slots hold.
    ///
    /// Cursors are free-running `i32`s compared only for equality and advanced
    /// with `wrapping_add`, so a value past `i32::MAX` is correct, not a bug —
    /// `RingIndex` masks them into the region. `sab_index` covers the wrap in
    /// `cursors_survive_wrapping_past_i32_max`.
    const fn as_word(bytes: u32) -> i32 {
        bytes as i32
    }

    /// Push one frame. Returns `false` when the ring has no room for it.
    ///
    /// Refusal is not loss: [`frame_queue::FrameQueue`] holds the frame in the
    /// producer's heap and retries. See the [`frame_queue::FrameSink`] impl
    /// below.
    #[must_use]
    pub fn push(&self, frame: &[u8]) -> bool {
        let Ok(len) = u32::try_from(frame.len()) else {
            return false;
        };
        let tail = self.load(Slot::Tail);
        let head = self.load(Slot::Head);
        let Some(reservation) = self.index.reserve(head, tail, len) else {
            return false;
        };
        if let Some(skip_at) = reservation.skip_at {
            self.words.set_index(skip_at / 4, SKIP_MARKER);
        }
        let at = reservation.at;
        self.words.set_index(at / 4, Self::as_word(len));
        self.data.subarray(at + 4, at + 4 + len).copy_from(frame);
        self.store(
            Slot::Tail,
            tail.wrapping_add(Self::as_word(reservation.advance)),
        );
        true
    }

    /// Pop one frame, or `None` when the ring is empty. Never blocks.
    #[must_use]
    pub fn pop(&self) -> Option<Vec<u8>> {
        loop {
            let head = self.load(Slot::Head);
            let tail = self.load(Slot::Tail);
            if head == tail {
                return None;
            }
            let start = self.index.offset(head);
            let word = self.words.get_index(start / 4);
            if word == SKIP_MARKER {
                self.store(
                    Slot::Head,
                    head.wrapping_add(Self::as_word(self.index.contiguous(head))),
                );
                continue;
            }
            let len = word.unsigned_abs();
            let frame = self.data.subarray(start + 4, start + 4 + len).to_vec();
            self.store(
                Slot::Head,
                head.wrapping_add(Self::as_word(self.index.frame_bytes(len))),
            );
            return Some(frame);
        }
    }
}

/// The ring as a bounded sink: a full ring refuses, and `FrameQueue` retries.
impl crate::transport::FrameSink for SabRing {
    fn offer(&self, frame: &[u8]) -> bool {
        self.push(frame)
    }
}
