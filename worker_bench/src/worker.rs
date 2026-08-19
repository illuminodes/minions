//! Web Worker entry point.
//!
//! Trunk builds this as `data-type="worker"`; it runs on a separate thread and
//! does nothing but register the relay reactor so the application can bridge
//! to it. All relay I/O, dedup, and filter-matching happen here, off the UI
//! thread.
//!
//! The reactor is `nostr_minions`' own — the same one the library ships to
//! consumers — so what this bench measures is what applications actually run.

fn main() {
    nostr_minions::relay_worker_main();
}
