//! The browser-bound half of the crate.
//!
//! Every module here binds to `web-sys`, `wasm-bindgen`, `yew`, or `idb`, so
//! the whole tree is gated to `wasm32` by the crate root. The parts that need
//! no browser live in [`crate::transport`], which is why the host can build
//! and run this crate's tests at all.

mod app_provider;
mod clock;
mod crypto;
mod idb_manager;
mod key_manager;
mod minion_error;
mod nostr_json;
mod relay_pool;

// Hooks
pub use idb_manager::{use_idb_database, use_idb_manager};
pub use key_manager::{
    use_create_local_key, use_delete_local_key, use_nostr_id_ctx, use_nostr_key, use_nostr_pubkey,
};
pub use relay_pool::{
    use_live_note, use_nostr_notes, use_nostr_relay_pool, use_notes_by_authors, use_notes_by_kind,
    use_recent_notes, use_relay_events, use_text_notes,
};

// The JSON + clock helpers, so consumers can encode Nostr types without caring
// which backend the build selected.
pub use clock::WallClock;
pub use nostr_json::NostrJson;

// Providers / components
pub use app_provider::{AppProps, NostrAppProvider};
pub use idb_manager::IdbManagerProvider;
pub use key_manager::{NostrIdProvider, NostrIdStore};
pub use relay_pool::{NostrRelayPoolProvider, NostrRelayPoolStore};

// Types
pub use idb_manager::{IdbStore, NostrIdb};
pub use key_manager::{GiftwrapScheme, IdbKeypairEntry, NostrId, NostrIdAction};
pub use minion_error::MinionError;
pub use relay_pool::{
    is_relay_worker, relay_worker_main, spawn_relay_bridge, BoundedDedup, JsonCodec, NoteIngestor,
    NostrRelayPool, NostrRelayPoolAction, ReadyState, RelayCommand, RelayEventSubscription,
    RelayReactor, SubscriptionId, SubscriptionInfo, TransportStatus, UserRelay, WorkerOut,
};

// The synthetic-load harness `worker_bench` drives. Off unless `bench-harness`
// is on, so a normal build carries none of it.
#[cfg(feature = "bench-harness")]
pub use relay_pool::{FloodRunner, FloodSpec, SyntheticFrame};

#[cfg(feature = "test-components")]
pub use key_manager::{NostrIdLoginTest, NostrIdLoginTestSuspense};
#[cfg(feature = "test-components")]
pub use relay_pool::RelayPoolTest;
