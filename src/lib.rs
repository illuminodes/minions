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
pub use idb_manager::*;
pub use key_manager::*;
pub use nostro2_signer::nostro2;
pub use relay_pool::*;
pub extern crate nostro2_signer;
pub use nostro2_signer::keypair::{EncryptionScheme, NostrKeypair};
pub use nostro2_signer::nostro2::*;

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
