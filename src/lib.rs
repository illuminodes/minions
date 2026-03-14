#![warn(
    clippy::all,
    clippy::missing_errors_doc,
    clippy::style,
    clippy::unseparated_literal_suffix,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::future_not_send, clippy::missing_errors_doc)]

pub mod browser_api;
mod idb_manager;
mod key_manager;
mod relay_pool;

// Re-export hooks
pub use idb_manager::{use_idb_database, use_idb_manager};
pub use key_manager::{
    use_create_local_key, use_delete_local_key, use_nostr_id_ctx, use_nostr_key, use_nostr_pubkey,
};
pub use relay_pool::{
    use_live_note, use_nostr_notes, use_nostr_relay_pool, use_notes_by_authors, use_notes_by_kind,
    use_recent_notes, use_text_notes,
};

// Re-export providers/components
pub use idb_manager::IdbManagerProvider;
pub use key_manager::{NostrIdProvider, NostrIdStore};
pub use relay_pool::{NostrRelayPoolProvider, NostrRelayPoolStore};

// Re-export types
pub use idb_manager::{IdbStore, NostrIdb};
pub use key_manager::{IdbKeypairEntry, NostrId, NostrIdAction};
pub use relay_pool::{NostrRelayPool, ReadyState, SubscriptionId, SubscriptionInfo, UserRelay};

// Re-export test components behind feature gate
#[cfg(feature = "test-components")]
pub use key_manager::{NostrIdLoginTest, NostrIdLoginTestSuspense};
#[cfg(feature = "test-components")]
pub use relay_pool::RelayPoolTest;

// Re-export nostro2 types directly for convenience
// This allows: use minions::NostrNote instead of minions::nostro2::NostrNote
pub use nostro2_signer::keypair::{EncryptionScheme, GiftwrapScheme, NostrKeypair};
pub use nostro2_signer::nostro2::*;

// Make the full crate available for advanced usage
// This allows: minions::nostro2_signer::... if needed
pub extern crate nostro2_signer;

#[derive(Clone, Debug, PartialEq, yew::Properties)]
pub struct AppProps {
    pub children: yew::html::Children,
    #[prop_or_default]
    pub relays: Vec<UserRelay>,
    #[prop_or_default]
    pub fallback: yew::html::Html,
}

#[yew::function_component(NostrAppProvider)]
pub fn nostr_app_provider(props: &AppProps) -> yew::Html {
    yew::html! {
        <yew::suspense::Suspense fallback={props.fallback.clone()}>
            <IdbManagerProvider>
                <NostrRelayPoolProvider relays={props.relays.clone()}>
                    <NostrIdProvider>
                        {props.children.clone()}
                    </NostrIdProvider>
                </NostrRelayPoolProvider>
            </IdbManagerProvider>
        </yew::suspense::Suspense>
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MinionError {
    #[error("IDB error: {0}")]
    Idb(#[from] idb::Error),
    #[error("Could not find Nostr key")]
    NoNostrKeyFound,
    #[error("Nostr Error: {0}")]
    NostrError(#[from] nostro2_signer::nostro2::errors::NostrErrors),
    #[error("Nostr Keypair Error: {0}")]
    NostrKeypairError(#[from] nostro2_signer::errors::NostrKeypairError),
    #[error("Crypto Error: {0:?}")]
    CryptoError(wasm_bindgen::JsValue),
    #[error("Nostr Relay Error: {0}")]
    WasmSerde(#[from] serde_wasm_bindgen::Error),
    #[error("No Identity Found")]
    NoIdentityFound,
}

impl From<MinionError> for web_sys::wasm_bindgen::JsValue {
    fn from(e: MinionError) -> Self {
        Self::from_str(e.to_string().as_str())
    }
}
