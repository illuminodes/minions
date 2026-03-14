#![warn(
    clippy::all,
    clippy::missing_errors_doc,
    clippy::style,
    clippy::unseparated_literal_suffix,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::future_not_send, clippy::missing_errors_doc)]

mod crypto;
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
    use_recent_notes, use_relay_events, use_text_notes,
};

// Re-export providers/components
pub use idb_manager::IdbManagerProvider;
pub use key_manager::{NostrIdProvider, NostrIdStore};
pub use relay_pool::{NostrRelayPoolProvider, NostrRelayPoolStore};

// Re-export types
pub use idb_manager::{IdbStore, NostrIdb};
pub use key_manager::{IdbKeypairEntry, NostrId, NostrIdAction};
pub use relay_pool::{
    NostrRelayPool, ReadyState, RelayEventSubscription, SubscriptionId, SubscriptionInfo, UserRelay,
};

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

#[derive(Debug)]
pub enum MinionError {
    Idb(idb::Error),
    NoNostrKeyFound,
    NostrError(nostro2_signer::nostro2::errors::NostrErrors),
    NostrKeypairError(nostro2_signer::errors::NostrKeypairError),
    CryptoError(wasm_bindgen::JsValue),
    WasmSerde(serde_wasm_bindgen::Error),
    NoIdentityFound,
}

impl std::fmt::Display for MinionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idb(e) => write!(f, "IDB error: {e}"),
            Self::NoNostrKeyFound => write!(f, "Could not find Nostr key"),
            Self::NostrError(e) => write!(f, "Nostr Error: {e}"),
            Self::NostrKeypairError(e) => write!(f, "Nostr Keypair Error: {e}"),
            Self::CryptoError(e) => write!(f, "Crypto Error: {e:?}"),
            Self::WasmSerde(e) => write!(f, "Serialization Error: {e}"),
            Self::NoIdentityFound => write!(f, "No Identity Found"),
        }
    }
}

impl std::error::Error for MinionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Idb(e) => Some(e),
            Self::NostrError(e) => Some(e),
            Self::NostrKeypairError(e) => Some(e),
            Self::WasmSerde(e) => Some(e),
            _ => None,
        }
    }
}

impl From<idb::Error> for MinionError {
    fn from(e: idb::Error) -> Self {
        Self::Idb(e)
    }
}

impl From<nostro2_signer::nostro2::errors::NostrErrors> for MinionError {
    fn from(e: nostro2_signer::nostro2::errors::NostrErrors) -> Self {
        Self::NostrError(e)
    }
}

impl From<nostro2_signer::errors::NostrKeypairError> for MinionError {
    fn from(e: nostro2_signer::errors::NostrKeypairError) -> Self {
        Self::NostrKeypairError(e)
    }
}

impl From<serde_wasm_bindgen::Error> for MinionError {
    fn from(e: serde_wasm_bindgen::Error) -> Self {
        Self::WasmSerde(e)
    }
}

impl From<MinionError> for web_sys::wasm_bindgen::JsValue {
    fn from(e: MinionError) -> Self {
        Self::from_str(e.to_string().as_str())
    }
}
