use nostro2_signer::nostro2::note::NostrNote;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use yew::{platform::spawn_local, prelude::*};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NostrId {
    loaded: bool,
    identity: Option<super::nostr_id::UserIdentity>,
    pubkey: Option<String>,
}
impl NostrId {
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    pub fn get_identity(&self) -> Option<&super::nostr_id::UserIdentity> {
        self.identity.as_ref()
    }
    pub fn get_pubkey(&self) -> Option<String> {
        self.pubkey.clone()
    }
    pub async fn sign_note(&self, note: &mut NostrNote) -> Result<(), JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or(JsValue::from_str("No identity"))?;
        id.sign_nostr_note(note).await
    }
    pub async fn sign_encrypted_note(
        &self,
        note: &mut NostrNote,
        pubkey: String,
    ) -> Result<(), JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or(JsValue::from_str("No identity"))?;
        id.sign_nip44(note, pubkey).await
    }
    pub async fn decrypt_note(&self, event: &NostrNote) -> Result<String, JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or(JsValue::from_str("No identity"))?;
        id.decrypt_nip44(event).await
    }
    pub async fn get_nostr_key(&self) -> Option<nostro2_signer::keypair::NostrKeypair> {
        let id = self.identity.as_ref()?;
        id.get_user_keys().await.ok()
    }
    pub async fn create_giftwrap(
        &self,
        inner_note: NostrNote,
        kind: u32,
    ) -> Result<NostrNote, JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or(JsValue::from_str("No identity"))?;
        id.create_giftwrap(inner_note, kind).await
    }

    pub async fn unwrap_giftwrap(&self, giftwrap: &NostrNote) -> Result<NostrNote, JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or(JsValue::from_str("No identity"))?;
        id.unwrap_giftwrap(giftwrap).await
    }
}

pub enum NostrIdAction {
    FinishedLoadingKey,
    LoadIdentity(String, super::nostr_id::UserIdentity),
    DeleteIdentity,
}
impl Reducible for NostrId {
    type Action = NostrIdAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            NostrIdAction::LoadIdentity(pubkey, id) => Rc::new(NostrId {
                loaded: true,
                pubkey: Some(pubkey),
                identity: Some(id),
            }),
            NostrIdAction::FinishedLoadingKey => Rc::new(NostrId {
                loaded: true,
                pubkey: self.pubkey.clone(),
                identity: self.identity.clone(),
            }),
            NostrIdAction::DeleteIdentity => Rc::new(NostrId {
                loaded: self.loaded,
                pubkey: None,
                identity: None,
            }),
        }
    }
}
pub type NostrIdStore = UseReducerHandle<NostrId>;

#[function_component(NostrIdProvider)]
pub fn key_handler(props: &yew::html::ChildrenProps) -> Html {
    let ctx = use_reducer(|| NostrId {
        loaded: false,
        pubkey: None,
        identity: None,
    });
    let ctx_clone = ctx.clone();
    use_memo((), move |_| {
        spawn_local(async move {
            match super::nostr_id::UserIdentity::find_identity().await {
                Ok(id) => {
                    match id.get_pubkey().await {
                        Some(pubkey) => {
                            ctx_clone.dispatch(NostrIdAction::LoadIdentity(pubkey, id));
                        }
                        None => {
                            gloo::console::error!("No pubkey found for identity");
                        }
                    }
                    ctx_clone.dispatch(NostrIdAction::FinishedLoadingKey);
                }
                Err(e) => {
                    gloo::console::error!("Error loading identity: ", e);
                    ctx_clone.dispatch(NostrIdAction::FinishedLoadingKey);
                }
            }
        });
    });

    // use_effect_with((), |_| || {});

    html! {
        <ContextProvider<NostrIdStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<NostrIdStore>>
    }
}
