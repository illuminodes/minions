use nostro2_signer::nostro2::{NostrNote, NostrSigner};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdbKeypairEntry {
    pub pubkey: String,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub keypair: web_sys::CryptoKey,
}
impl IdbKeypairEntry {
    pub async fn from_keypair(
        keypair: nostro2_signer::keypair::NostrKeypair,
    ) -> Result<Self, crate::MinionError> {
        let array = keypair.secret_key();
        let js_array = web_sys::js_sys::Uint8Array::from(array.as_slice());
        let crypto_key: web_sys::CryptoKey = crate::browser_api::BrowserCrypto::default()
            .import_key_array(js_array.into())
            .await?;
        Ok(Self {
            pubkey: keypair.public_key(),
            keypair: crypto_key,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NostrId {
    identity: Option<nostro2_signer::keypair::NostrKeypair>,
    pubkey: Option<String>,
}
impl NostrId {
    #[must_use]
    pub fn get_pubkey(&self) -> Option<String> {
        self.pubkey.clone()
    }
    pub fn sign_note(&self, note: &mut NostrNote) -> Result<(), crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(id.sign_nostr_note(note)?)
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
        Ok(id.sign_encrypted_note(
            note,
            pubkey,
            &nostro2_signer::keypair::EncryptionScheme::Nip44,
        )?)
    }
    pub fn decrypt_note(&self, event: &NostrNote) -> Result<String, crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(id
            .decrypt_note(
                event,
                event.pubkey.as_str(),
                &nostro2_signer::keypair::EncryptionScheme::Nip44,
            )?
            .to_string())
    }
    #[must_use]
    pub const fn get_nostr_key(&self) -> Option<&nostro2_signer::keypair::NostrKeypair> {
        self.identity.as_ref()
    }
    pub fn create_giftwrap(
        &self,
        inner_note: &mut NostrNote,
        peer_pubkey: &str,
        scheme: &nostro2_signer::keypair::GiftwrapScheme,
    ) -> Result<NostrNote, crate::MinionError> {
        let id = self
            .identity
            .as_ref()
            .ok_or(crate::MinionError::NoNostrKeyFound)?;
        Ok(id.giftwrap_note(inner_note, peer_pubkey, scheme)?)
    }
}

pub enum NostrIdAction {
    LoadIdentity(String, nostro2_signer::keypair::NostrKeypair),
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
pub fn key_handler(props: &yew::html::ChildrenProps) -> HtmlResult {
    let idb = crate::idb_manager::use_idb_database();
    let identity =
        yew::suspense::use_future_with((), |_| async move { idb.load_identity().await })?;
    let ctx = use_reducer(|| NostrId {
        identity: identity.as_ref().cloned().ok().flatten(),
        pubkey: identity.as_ref().ok().and_then(|id| {
            id.as_ref()
                .map(nostro2_signer::keypair::NostrKeypair::public_key)
        }),
    });

    Ok(html! {
       <ContextProvider<NostrIdStore> context={ctx}>
           {props.children.clone()}
       </ContextProvider<NostrIdStore>>
    })
}
