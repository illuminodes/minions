use nostro2_signer::nostro2::NostrNote;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NostrId {
    loaded: bool,
    identity: Option<super::nostr_id::UserIdentity>,
    pubkey: Option<String>,
}
impl NostrId {
    #[must_use]
    pub const fn loaded(&self) -> bool {
        self.loaded
    }
    #[must_use]
    pub const fn get_identity(&self) -> Option<&super::nostr_id::UserIdentity> {
        self.identity.as_ref()
    }
    #[must_use]
    pub fn get_pubkey(&self) -> Option<String> {
        self.pubkey.clone()
    }
    pub async fn sign_note(&self, note: &mut NostrNote) -> Result<(), JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or_else(|| JsValue::from_str("No identity"))?;
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
            .ok_or_else(|| JsValue::from_str("No identity"))?;
        id.sign_nip44(note, pubkey).await
    }
    pub async fn decrypt_note(&self, event: &NostrNote) -> Result<String, JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or_else(|| JsValue::from_str("No identity"))?;
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
            .ok_or_else(|| JsValue::from_str("No identity"))?;
        id.create_giftwrap(inner_note, kind).await
    }

    pub async fn unwrap_giftwrap(&self, giftwrap: &NostrNote) -> Result<NostrNote, JsValue> {
        let id = self
            .identity
            .as_ref()
            .ok_or_else(|| JsValue::from_str("No identity"))?;
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

    let ctx_clone = ctx.dispatcher();
    use_memo((), move |()| {
        yew::platform::spawn_local(async move {
            let id = super::nostr_id::UserIdentity::find_identity().await;
            if let Ok(user_id) = id {
                ctx_clone.dispatch(NostrIdAction::LoadIdentity(
                    user_id.get_pubkey().await.unwrap_or_default(),
                    user_id.clone(),
                ));
                return;
            }
            ctx_clone.dispatch(NostrIdAction::FinishedLoadingKey);
        });
    });

    html! {
       <ContextProvider<NostrIdStore> context={ctx}>
           {props.children.clone()}
       </ContextProvider<NostrIdStore>>
    }
}
