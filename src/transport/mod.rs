//! The pool's wire language and the pure arithmetic behind its shared ring.
//!
//! # Why this is one module tree
//!
//! Every module below is plain Rust: no `web-sys`, no `wasm-bindgen`, no
//! `yew`. That is deliberate and load-bearing. The rest of this crate binds to
//! browser APIs and therefore only compiles for `wasm32-unknown-unknown`,
//! where `cargo test` has no runner. These are the parts where a bug is an
//! off-by-one or a truncated frame rather than a DOM mistake, so they are kept
//! host-clean and carry the crate's executable tests.
//!
//! Off `wasm32` the crate root compiles this tree ALONE, so
//! `cargo test --target x86_64-unknown-linux-gnu` builds and runs these tests.
//!
//! # What lives here
//!
//! - [`Envelope`] with [`Inbound`] / [`Outbound`]: every message the pool
//!   sends, in one tagged frame format.
//! - [`NoteCodec`] over [`Reader`] / [`Writer`]: the flat note encoding that
//!   keeps a JSON parse off the UI thread.
//! - [`RingIndex`] with [`Reservation`]: head/tail arithmetic for the byte
//!   ring.
//! - [`FrameQueue`] over [`FrameSink`]: the loss-free spill buffer in front of
//!   a bounded ring.
//!
//! The message vocabulary is NOT feature-gated: those types are the pool's own
//! language in both the bridge and the shared-ring build, so the worker keeps
//! one code path either way. Only the ring pieces answer to `sab-transport`.

mod envelope;
mod frame_queue;
mod note_codec;
mod reader;
mod ring_index;
mod writer;

pub use envelope::{Envelope, Inbound, Outbound};
pub use frame_queue::{FrameQueue, FrameSink};
pub use note_codec::NoteCodec;
pub use reader::Reader;
pub use ring_index::{Reservation, RingIndex};
pub use writer::Writer;
