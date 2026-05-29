//! Web Worker entry point.
//!
//! Trunk builds this as `data-type="worker"`; it runs on a separate thread and
//! does nothing but register the relay reactor so the application can bridge
//! to it. All relay I/O, dedup, and filter-matching happen here, off the UI
//! thread.

#[path = "relay_worker.rs"]
mod relay_worker;

use relay_worker::RelayReactor;
use yew_agent::Registrable;

fn main() {
    RelayReactor::registrar().register();
}
