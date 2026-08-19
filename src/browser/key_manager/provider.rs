use nostro2::{NostrNote, NostrSigner};
use nostro2_nips::Nip44;
use nostro2_signer::NostrKeypair;
use std::rc::Rc;
use yew::prelude::*;

use super::GiftwrapScheme;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdbKeypairEntry {
    pub pubkey: String,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub keypair: web_sys::CryptoKey,
}
impl IdbKeypairEntry {
    pub async fn from_keypair(keypair: NostrKeypair) -> Result<Self, crate::MinionError> {
        use nostro2::NostrKeypair as _;
        let array = keypair.secret_bytes();
        let js_array = web_sys::js_sys::Uint8Array::from(array.as_slice());
        let crypto_key: web_sys::CryptoKey =
            crate::browser::crypto::import_key_array(js_array.into()).await?;
        Ok(Self {
            pubkey: keypair.public_key(),
            keypair: crypto_key,
        })
    }
}

/// The signed-in identity.
///
/// `NostrKeypair` is opaque (no `PartialEq`), so equality — which Yew needs to
/// decide whether context consumers re-render — compares the public key only.
/// Two `NostrId`s with the same pubkey hold the same secret by construction.
#[derive(Clone, Debug)]
pub struct NostrId {
    identity: Option<NostrKeypair>,
    pubkey: Option<String>,
}

impl PartialEq for NostrId {
    fn eq(&self, other: &Self) -> bool {
        self.pubkey == other.pubkey
    }
}

impl Eq for NostrId {}

impl NostrId {
    #[must_use]
    pub fn get_pubkey(&self) -> Option<&str> {
        self.pubkey.as_deref()
    }
    pub fn sign_note(&self, note: &mut NostrNote) -> Result<(), crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(note.sign_with(id)?)
    }
    pub fn sign_encrypted_note(
        &self,
        note: &mut NostrNote,
        pubkey: &str,
    ) -> Result<(), crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        id.nip44_encrypt_note(note, pubkey)?;
        Ok(note.sign_with(id)?)
    }
    pub fn decrypt_note(&self, event: &NostrNote) -> Result<String, crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(id
            .nip44_decrypt_note(event, event.pubkey.as_str())?
            .to_string())
    }
    #[must_use]
    pub const fn get_nostr_key(&self) -> Option<&NostrKeypair> {
        self.identity.as_ref()
    }
    pub fn create_giftwrap(
        &self,
        inner_note: &mut NostrNote,
        peer_pubkey: &str,
        scheme: GiftwrapScheme,
    ) -> Result<NostrNote, crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(scheme.wrap(id, inner_note, peer_pubkey)?)
    }
}

pub enum NostrIdAction {
    LoadIdentity(String, NostrKeypair),
    DeleteIdentity,
}
impl Reducible for NostrId {
    type Action = NostrIdAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            NostrIdAction::LoadIdentity(pubkey, id) => Rc::new(Self {
                pubkey: Some(pubkey),
                identity: Some(id),
            }),
            NostrIdAction::DeleteIdentity => Rc::new(Self {
                pubkey: None,
                identity: None,
            }),
        }
    }
}
pub type NostrIdStore = UseReducerHandle<NostrId>;

#[function_component(NostrIdProvider)]
pub fn nostr_id_provider(props: &yew::html::ChildrenProps) -> HtmlResult {
    let idb = crate::browser::idb_manager::use_idb_database();
    let identity = yew::suspense::use_future_with((), |_| async move {
        match idb {
            Some(idb) => idb.load_identity().await,
            None => Ok(None),
        }
    })?;
    let ctx = use_reducer(|| NostrId {
        identity: identity.as_ref().cloned().ok().flatten(),
        pubkey: identity
            .as_ref()
            .ok()
            .and_then(|id| id.as_ref().map(NostrSigner::public_key)),
    });

    Ok(html! {
       <ContextProvider<NostrIdStore> context={ctx}>
           {props.children.clone()}
       </ContextProvider<NostrIdStore>>
    })
}
