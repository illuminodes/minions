//! Phase 2: a quetzalcoatl SPSC ring of `NostrNote` living in the shared wasm
//! heap, written by the worker and read by the main thread — zero-copy IPC,
//! no `postMessage` per note.
//!
//! Why this is sound even though `NostrNote` holds `String`s (heap pointers):
//! the worker and main thread run the SAME module against the SAME shared
//! linear memory, so they share ONE std allocator (dlmalloc, locked under
//! `+atomics`). A `String` allocated by the worker is valid on the main thread
//! because both allocate from the same shared heap. The ring slots and the
//! note payloads they point at all live in that one shared memory.
//!
//! Lifetime: the ring is `Box::leak`ed (process-lifetime, never moved/dropped),
//! which is exactly the contract `quetzalcoatl`'s `producer_from_raw` /
//! `consumer_from_raw` require.

// Reuse the single metrics module owned by `relay_worker` (loading metrics.rs
// again here via #[path] would compile it twice in this bin — duplicate_mod).
use crate::relay_worker::metrics;

use nostro2::{NostrNote, NostrRelayEvent, NostrSubscription};
use quetzalcoatl::capacity::Capacity;
use quetzalcoatl::spsc::RingBuffer;
use wasm_bindgen::prelude::*;

/// Ring capacity (power of two). Sized for the WORST payload case, not the
/// best: at 20 KB/note a 16k-slot ring could pin ~320 MB of in-flight heap if
/// the consumer falls behind, which overflows shared memory and aborts the page
/// (a failed wasm `memory.grow` traps, it doesn't return an error). 1024 slots
/// bounds that to ~20 MB at 20 KB/note. When the ring is full the producer
/// DROPS rather than blocks (see `ring_worker_main`), so a small ring just means
/// drops-under-overload, which the metrics surface honestly.
const RING_CAP: usize = 1024;

/// Shared ring + a shared "dropped" counter, allocated together.
///
/// `dropped` is an `AtomicU64` in the shared heap the worker bumps on a full
/// ring and the main thread reads. Both are leaked (process-lifetime).
/// Returns `(ring_ptr, dropped_ptr, running_ptr)`. `running` is a shared
/// `AtomicBool` the worker holds for the duration of a flood; the main thread
/// gates new floods on it so the SPSC ring never has two producers.
#[must_use]
pub fn alloc_ring() -> (usize, usize, usize) {
    let ring: &'static RingBuffer<NostrNote> =
        Box::leak(Box::new(RingBuffer::new(Capacity::exact(RING_CAP))));
    let dropped: &'static std::sync::atomic::AtomicU64 =
        Box::leak(Box::new(std::sync::atomic::AtomicU64::new(0)));
    let running: &'static std::sync::atomic::AtomicBool =
        Box::leak(Box::new(std::sync::atomic::AtomicBool::new(false)));
    (
        std::ptr::from_ref(ring) as usize,
        std::ptr::from_ref(dropped) as usize,
        std::ptr::from_ref(running) as usize,
    )
}

/// Read the shared "flood running" flag (main thread). `true` while a worker
/// flood is in progress.
///
/// # Safety
/// `ptr` must be the running-flag address from [`alloc_ring`] (leaked, `'static`).
#[must_use]
pub unsafe fn is_running(ptr: usize) -> bool {
    if ptr == 0 {
        return false;
    }
    // SAFETY: caller passes the leaked AtomicBool address from alloc_ring.
    let running: &std::sync::atomic::AtomicBool = unsafe { &*(ptr as *const _) };
    running.load(std::sync::atomic::Ordering::Acquire)
}

/// Read the shared dropped-note counter (main thread).
///
/// # Safety
/// `ptr` must be the dropped-counter address from [`alloc_ring`] (leaked,
/// `'static`).
#[must_use]
pub unsafe fn dropped_count(ptr: usize) -> u64 {
    if ptr == 0 {
        return 0;
    }
    // SAFETY: caller passes the leaked AtomicU64 address from alloc_ring.
    let dropped: &std::sync::atomic::AtomicU64 = unsafe { &*(ptr as *const _) };
    dropped.load(std::sync::atomic::Ordering::Relaxed)
}

/// Build the consumer handle (main thread) from the ring address.
///
/// # Safety
/// `ptr` must be the address returned by [`alloc_ring`] for a ring that is
/// still alive (it is leaked, so always), and there must be exactly one
/// consumer (this one) and one producer (in the worker).
#[must_use]
pub unsafe fn consumer(
    ptr: usize,
) -> quetzalcoatl::spsc::Consumer<NostrNote, &'static RingBuffer<NostrNote>> {
    // SAFETY: caller upholds the alloc_ring + single-consumer contract.
    unsafe { RingBuffer::consumer_from_raw(ptr as *const RingBuffer<NostrNote>) }
}

/// Worker entry: build the producer from the shared ring address and run the
/// synthetic flood, pushing matched notes into the ring.
///
/// Exported so the worker bootstrap can call it after `initSync`. `rate` /
/// `secs` / `payload_bytes` mirror the benchmark flood knobs; `filter_json` is
/// a serialized `NostrSubscription` the notes are matched against. `dropped_ptr`
/// is the shared `AtomicU64` from `alloc_ring` bumped when the ring is full.
#[wasm_bindgen]
pub fn ring_worker_main(
    ring_ptr: usize,
    // The shared drop counter is no longer bumped: the producer below uses the
    // BLOCKING push and never drops. Kept in the signature (and left at 0) so the
    // main thread's args/plumbing and the "dropped" metric stay intact — the
    // metric now correctly reports 0 for this path.
    _dropped_ptr: usize,
    running_ptr: usize,
    rate: u32,
    secs: u32,
    payload_bytes: usize,
    filter_json: String,
) {
    // SAFETY: main thread leaked the ring at `ring_ptr`; we are the sole producer.
    let producer = unsafe {
        RingBuffer::<NostrNote>::producer_from_raw(ring_ptr as *const RingBuffer<NostrNote>)
    };
    // Cooperative "a flood is running" flag (shared AtomicBool). We set it true
    // for the duration and clear it on exit. The main thread refuses to start a
    // second flood while it's true, guaranteeing the SPSC ring has exactly ONE
    // producer at a time. We NEVER terminate() the worker from the main thread:
    // killing a thread mid-`malloc` orphans the shared dlmalloc lock and hangs
    // every later allocation (the "page unresponsive" we saw). Cooperative exit
    // is the only safe shutdown for a thread sharing the allocator.
    let running: &'static std::sync::atomic::AtomicBool =
        unsafe { &*(running_ptr as *const std::sync::atomic::AtomicBool) };
    running.store(true, std::sync::atomic::Ordering::Release);

    let filter: NostrSubscription =
        serde_json::from_str(&filter_json).unwrap_or(NostrSubscription {
            kinds: Some(vec![1]),
            ..Default::default()
        });

    let mut dedup = nostr_minions::BoundedDedup::new(10_000);

    // FULLY SYNCHRONOUS generation — no `spawn_local`, no async `sleep`.
    // Critical: this worker only ran `initSync` (it instantiated the wasm
    // module against shared memory); it did NOT start a Yew/prokio runtime, so
    // `yew::platform::spawn_local` / `time::sleep` have no executor to run on.
    // Calling them here panicked the worker — and a panic while it holds the
    // shared dlmalloc lock (it allocates constantly) orphans that lock, hanging
    // the MAIN thread on its next allocation. That was the instant freeze.
    //
    // A plain loop needs no runtime. It generates the whole flood in one go;
    // the bounded ring (drop-on-full) absorbs the burst and the main thread
    // drains at its own rAF pace. Total iterations are bounded (rate*secs), so
    // the worker returns cleanly and clears the running flag.
    let total = u64::from(rate) * u64::from(secs);
    for seq in 0..total {
        // Stamp the emit time PER NOTE, immediately before generating it. With
        // the blocking push below, the worker no longer produces the whole flood
        // in one instant burst — it parks whenever the ring is full and resumes
        // as the consumer drains, so generation now spans real wall-clock time.
        // A single pre-loop `emit` would then make every note's measured latency
        // include the entire flood duration (massively inflating p95/p99). A
        // per-note stamp keeps end-to-end latency honest: it measures time spent
        // waiting in the ring + cross-thread handoff, not the flood's runtime.
        let emit = metrics::wall_ms();
        let raw = metrics::synthetic_event(seq, emit, payload_bytes);
        // SAME ingestion path as a real relay onmessage: parse → dedup → filter.
        let Ok(NostrRelayEvent::NewNote(.., note)) = raw.parse::<NostrRelayEvent>() else {
            continue;
        };
        if let Some(ref id) = note.id {
            if !dedup.insert(id.clone()) {
                continue;
            }
        }
        if !nostr_minions::note_matches_filter(&note, &filter) {
            continue;
        }
        // BLOCKING push: never drop. When the ring is full the producer parks
        // (std::thread::park → memory.atomic.wait; legal here because this is a
        // worker thread, never the main thread). The main-thread consumer's pop
        // calls wake_producer (Atomics.notify) after making space, unparking us.
        // Because the consumer drains on its own rAF — independent of this worker
        // blocking — there is no deadlock: the worker stalls only as long as the
        // ring stays full, then resumes. Every matched note is therefore handled.
        //
        // push_block returns Err ONLY if the consumer has been dropped (the
        // RingPanel unmounted / path switched away). There is no one left to
        // drain, so blocking would hang forever — stop the flood cleanly instead.
        if producer.push_block(note).is_err() {
            break;
        }
    }
    // Clear the running flag so the main thread can start the next flood.
    running.store(false, std::sync::atomic::Ordering::Release);
}
