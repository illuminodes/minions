//! Path 4: a byte ring inside a `SharedArrayBuffer`, synchronized with
//! `js_sys::Atomics`.
//!
//! This is the STABLE-TOOLCHAIN transport. Unlike `ring.rs`, nothing here
//! depends on shared wasm linear memory, on `-Z build-std`, or on a pointer
//! being valid across threads. The two sides share ONE JS object and exchange
//! plain bytes, so each side could run its own ordinary non-shared wasm memory
//! built by stable rustc.
//!
//! What it costs versus `ring.rs`: the notes are serialized. What it keeps:
//! no `postMessage` per note (no structured clone, no event-loop task per
//! note) and batch draining.
//!
//! Frames carry [`NoteCodec`]'s flat binary encoding, NOT the relay's raw
//! `["EVENT",..]` text. Forwarding the raw text was the first design, and it
//! measured badly: the consumer re-parsed on the UI thread what the producer
//! had already parsed, which cost 11% jank against the shared-memory ring's
//! 0.1% at 2900 notes/s. The producer must parse anyway to dedup and filter,
//! so it writes the parsed fields out flat and the consumer reads them back
//! with bounds-checked slicing.
//!
//! Layout of the buffer:
//!
//! - bytes `0..16` — four `i32` header slots, addressed by [`Slot`]
//! - bytes `16..16+cap` — the data region, a power-of-two byte ring
//!
//! A frame is a 4-byte little-endian length followed by that many payload
//! bytes, padded so every frame starts 4-byte aligned. A length of `-1` is a
//! skip marker meaning "no more frames before the end of the region, resume at
//! offset 0"; it keeps every frame contiguous so each one moves in a single
//! typed-array copy.
//!
//! Only the producer advances `Tail` and only the consumer advances `Head`, so
//! this is a single-producer/single-consumer ring. The atomic head/tail stores
//! order the non-atomic payload reads and writes around them.

use wasm_bindgen::prelude::*;

use nostro2::{NostrRelayEvent, NostrSubscription};
use web_sys::js_sys::{Atomics, Int32Array, SharedArrayBuffer, Uint8Array};

use crate::metrics;
use nostr_minions::transport::NoteCodec;
use nostr_minions::transport::RingIndex;

const HEADER_BYTES: u32 = 16;
/// Data region size. Sized to hold several of the largest payloads the bench
/// offers (1 MB/note) so a big-payload run measures the transport rather than
/// the ring being permanently full.
const CAP_BYTES: u32 = 1 << 23;

/// Header slots, in `i32` units from the start of the buffer.
#[derive(Clone, Copy)]
enum Slot {
    Head = 0,
    Tail = 1,
    Dropped = 2,
    ConsumerAlive = 3,
}

impl Slot {
    const fn index(self) -> u32 {
        self as u32
    }
}

/// Both ends of the shared byte ring. The same struct serves the producer (in
/// the worker) and the consumer (on the main thread); which methods you call
/// decides the role.
pub struct SabRing {
    buffer: SharedArrayBuffer,
    header: Int32Array,
    words: Int32Array,
    data: Uint8Array,
    index: RingIndex,
}

impl SabRing {
    /// Allocate a new buffer and view it (main thread).
    ///
    /// # Errors
    /// Returns a `JsValue` if the environment refuses to allocate a
    /// `SharedArrayBuffer` (it is absent unless the page is cross-origin
    /// isolated).
    pub fn create() -> Result<Self, JsValue> {
        let buffer = SharedArrayBuffer::new(HEADER_BYTES + CAP_BYTES);
        Self::view(buffer)
    }

    /// View a buffer received from the other thread (worker side).
    ///
    /// # Errors
    /// Returns a `JsValue` if `value` is not a `SharedArrayBuffer`.
    pub fn attach(value: JsValue) -> Result<Self, JsValue> {
        let buffer = value
            .dyn_into::<SharedArrayBuffer>()
            .map_err(|_| JsValue::from_str("sab: expected a SharedArrayBuffer"))?;
        Self::view(buffer)
    }

    fn view(buffer: SharedArrayBuffer) -> Result<Self, JsValue> {
        let raw: &JsValue = buffer.as_ref();
        let header = Int32Array::new_with_byte_offset_and_length(raw, 0, 4);
        let words = Int32Array::new_with_byte_offset_and_length(raw, HEADER_BYTES, CAP_BYTES / 4);
        let data = Uint8Array::new_with_byte_offset_and_length(raw, HEADER_BYTES, CAP_BYTES);
        Ok(Self {
            buffer,
            header,
            words,
            data,
            index: RingIndex::new(CAP_BYTES),
        })
    }

    /// The underlying buffer, to hand to the worker via `postMessage`.
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

    /// Mark the consumer present or gone. The producer stops when it reads
    /// `false`, so a worker parked on a full ring cannot outlive the panel that
    /// drains it.
    pub fn set_consumer_alive(&self, alive: bool) {
        self.store(Slot::ConsumerAlive, i32::from(alive));
    }

    #[must_use]
    pub fn consumer_alive(&self) -> bool {
        self.load(Slot::ConsumerAlive) != 0
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.load(Slot::Dropped).unsigned_abs().into()
    }

    /// Push one frame. Returns `false` when the ring has no room for it.
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
            self.words.set_index(skip_at / 4, -1);
        }
        let at = reservation.at;
        self.words.set_index(at / 4, len as i32);
        self.data.subarray(at + 4, at + 4 + len).copy_from(frame);
        self.store(Slot::Tail, tail.wrapping_add(reservation.advance as i32));
        true
    }

    /// Push one frame, parking while the ring is full. Returns `false` if the
    /// consumer went away, which means no one will ever make room.
    #[must_use]
    pub fn push_block(&self, frame: &[u8]) -> bool {
        loop {
            if self.push(frame) {
                return true;
            }
            if !self.consumer_alive() {
                return false;
            }
            let head = self.load(Slot::Head);
            let _ = Atomics::wait_with_timeout(&self.header, Slot::Head.index(), head, 50.0);
        }
    }

    /// Pop one frame, or `None` when the ring is empty.
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
            if word < 0 {
                self.store(
                    Slot::Head,
                    head.wrapping_add(self.index.contiguous(head) as i32),
                );
                continue;
            }
            let len = word.unsigned_abs();
            let frame = self.data.subarray(start + 4, start + 4 + len).to_vec();
            self.store(
                Slot::Head,
                head.wrapping_add(self.index.frame_bytes(len) as i32),
            );
            return Some(frame);
        }
    }
}

/// The producer side of the SAB path: generate the synthetic flood, run the
/// real ingestion (parse, dedup, filter), and push matched raw frames.
struct SabFlood {
    ring: SabRing,
    filter: NostrSubscription,
    dedup: nostr_minions::BoundedDedup,
}

impl SabFlood {
    fn new(ring: SabRing, filter_json: &str) -> Self {
        let filter = serde_json::from_str(filter_json).unwrap_or(NostrSubscription {
            kinds: Some([1].into()),
            ..Default::default()
        });
        Self {
            ring,
            filter,
            dedup: nostr_minions::BoundedDedup::new(10_000),
        }
    }

    fn run(&mut self, rate: u32, secs: u32, payload_bytes: usize) {
        let total = u64::from(rate) * u64::from(secs);
        for seq in 0..total {
            let emit = metrics::wall_ms();
            let raw = metrics::synthetic_event(seq, emit, payload_bytes);
            let Ok(NostrRelayEvent::NewNote(.., note)) = raw.parse::<NostrRelayEvent>() else {
                continue;
            };
            if let Some(ref id) = note.id {
                if !self.dedup.insert(id.clone()) {
                    continue;
                }
            }
            if !self.filter.matches(&note) {
                continue;
            }
            if !self.ring.push_block(&NoteCodec::encode(&note)) {
                break;
            }
        }
    }
}

/// Worker entry for the SAB path. Thin FFI shim over [`SabFlood`]; the boot
/// module calls it by name after `initSync`.
#[wasm_bindgen]
pub fn sab_worker_main(
    sab: JsValue,
    rate: u32,
    secs: u32,
    payload_bytes: usize,
    filter_json: String,
) {
    let Ok(ring) = SabRing::attach(sab) else {
        return;
    };
    SabFlood::new(ring, &filter_json).run(rate, secs, payload_bytes);
}
