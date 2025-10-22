mod provider;
pub use provider::*;

mod tests;
pub use tests::*;

#[yew::hook]
pub fn use_nostr_id_ctx() -> provider::NostrIdStore {
    yew::use_context::<provider::NostrIdStore>().expect("NostrIdStore context")
}

#[yew::hook]
pub fn use_nostr_key() -> Option<nostro2_signer::keypair::NostrKeypair> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>().expect("NostrIdStore context");
    key_ctx.get_nostr_key().cloned()
}

#[yew::hook]
pub fn use_nostr_pubkey() -> Option<String> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    key_ctx.get_pubkey()
}

#[yew::hook]
pub fn use_create_local_key() -> yew::callback::Callback<nostro2_signer::keypair::NostrKeypair> {
    let idb_ctx = crate::use_idb_database();
    let key_ctx = yew::use_context::<provider::NostrIdStore>().expect("NostrIdStore context");
    yew::callback::Callback::from(move |keypair: nostro2_signer::keypair::NostrKeypair| {
        let idb_ctx = idb_ctx.clone();
        let key_ctx = key_ctx.clone();

        yew::platform::spawn_local(async move {
            let Ok(entry) = crate::IdbKeypairEntry::from_keypair(keypair.clone()).await else {
                return;
            };
            let Ok(()) = idb_ctx.add_identity(entry).await else {
                return;
            };
            key_ctx.dispatch(provider::NostrIdAction::LoadIdentity(
                keypair.public_key(),
                keypair,
            ));
        });
    })
}

#[yew::hook]
pub fn use_delete_local_key() -> yew::callback::Callback<()> {
    let idb_ctx = crate::use_idb_database();
    let key_ctx = use_nostr_id_ctx();
    yew::callback::Callback::from(move |()| {
        let idb_ctx = idb_ctx.clone();
        let key_dispatch = key_ctx.dispatcher();
        let Some(key) = key_ctx.get_nostr_key().cloned() else {
            return;
        };
        yew::platform::spawn_local(async move {
            let Ok(()) = idb_ctx.remove_identity(key.public_key()).await else {
                return;
            };
            key_dispatch.dispatch(provider::NostrIdAction::DeleteIdentity);
        });
    })
}
