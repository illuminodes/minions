/// Every failure this crate reports to its consumer.
#[derive(Debug)]
pub enum MinionError {
    Idb(idb::Error),
    NoNostrKeyFound,
    NostrError(nostro2::errors::NostrErrors),
    NostrKeypairError(nostro2_signer::errors::NostrKeypairError),
    Nip44(nostro2_nips::Nip44Error),
    Nip59(nostro2_nips::Nip59Error),
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
            Self::Nip44(e) => write!(f, "NIP-44 Error: {e}"),
            Self::Nip59(e) => write!(f, "NIP-59 Error: {e}"),
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
            Self::Nip44(e) => Some(e),
            Self::Nip59(e) => Some(e),
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

impl From<nostro2::errors::NostrErrors> for MinionError {
    fn from(e: nostro2::errors::NostrErrors) -> Self {
        Self::NostrError(e)
    }
}

impl From<nostro2_nips::Nip44Error> for MinionError {
    fn from(e: nostro2_nips::Nip44Error) -> Self {
        Self::Nip44(e)
    }
}

impl From<nostro2_nips::Nip59Error> for MinionError {
    fn from(e: nostro2_nips::Nip59Error) -> Self {
        Self::Nip59(e)
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
