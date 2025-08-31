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
    loaded: bool,
    identity: Option<nostro2_signer::keypair::NostrKeypair>,
    pubkey: Option<String>,
}
impl NostrId {
    #[must_use]
    pub const fn loaded(&self) -> bool {
        self.loaded
    }
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
    FinishedLoadingKey,
    LoadIdentity(String, nostro2_signer::keypair::NostrKeypair),
    LoadedNoId,
    DeleteIdentity,
}
impl Reducible for NostrId {
    type Action = NostrIdAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            NostrIdAction::LoadIdentity(pubkey, id) => Rc::new(Self {
                loaded: true,
                pubkey: Some(pubkey),
                identity: Some(id),
            }),
            NostrIdAction::FinishedLoadingKey => Rc::new(Self {
                loaded: true,
                pubkey: self.pubkey.clone(),
                identity: self.identity.clone(),
            }),
            NostrIdAction::DeleteIdentity => Rc::new(Self {
                loaded: self.loaded,
                pubkey: None,
                identity: None,
            }),
            NostrIdAction::LoadedNoId => Rc::new(Self {
                loaded: true,
                pubkey: self.pubkey.clone(),
                identity: None,
            }),
        }
    }
}
pub type NostrIdStore = UseReducerHandle<NostrId>;

pub async fn load_identity(
    db: std::rc::Rc<idb::Database>,
) -> Result<nostro2_signer::keypair::NostrKeypair, crate::MinionError> {
    let transaction = db.transaction(
        &[crate::idb_manager::NostrDbStoreName::UserIdentity.as_ref()],
        idb::TransactionMode::ReadOnly,
    )?;
    let store =
        transaction.object_store(crate::idb_manager::NostrDbStoreName::UserIdentity.as_ref())?;
    let keys = store.get_all(None, Some(1))?.await?;
    let Some(keys) = keys
        .into_iter()
        .next()
        .and_then(|key| serde_wasm_bindgen::from_value::<crate::IdbKeypairEntry>(key).ok())
    else {
        return Err(crate::MinionError::NoNostrKeyFound);
    };

    let crypto = crate::browser_api::BrowserCrypto::default();
    let secret_array = crypto.export_raw_key(keys.keypair).await?;
    let secret_slice = web_sys::js_sys::Uint8Array::new(&secret_array);
    Ok(nostro2_signer::keypair::NostrKeypair::try_from(
        secret_slice.to_vec().as_slice(),
    )?)
}

#[function_component(NostrIdProvider)]
pub fn key_handler(props: &yew::html::ChildrenProps) -> Html {
    let idb_ctx = crate::idb_manager::use_idb_manager();
    let ctx = use_reducer(|| NostrId {
        loaded: false,
        pubkey: None,
        identity: None,
    });

    let ctx_clone = ctx.dispatcher();
    use_memo(idb_ctx, move |idb_ctx| {
        let Some(db) = idb_ctx.as_ref().and_then(|ctx| (ctx.db.clone())) else {
            return;
        };
        yew::platform::spawn_local(async move {
            let identity = match load_identity(db).await {
                Ok(id) => id,
                Err(e) => {
                    web_sys::console::error_1(&format!("Error loading identity: {e:#?}").into());
                    ctx_clone.dispatch(NostrIdAction::LoadedNoId);
                    return;
                }
            };
            ctx_clone.dispatch(NostrIdAction::LoadIdentity(identity.public_key(), identity));
        });
    });

    html! {
       <ContextProvider<NostrIdStore> context={ctx}>
           {props.children.clone()}
       </ContextProvider<NostrIdStore>>
    }
}
