mod giftwrap;
mod provider;
pub use giftwrap::GiftwrapScheme;
pub use provider::{IdbKeypairEntry, NostrId, NostrIdAction, NostrIdProvider, NostrIdStore};

#[cfg(feature = "test-components")]
mod tests;
#[cfg(feature = "test-components")]
pub use tests::*;

#[yew::hook]
pub fn use_nostr_id_ctx() -> Option<provider::NostrIdStore> {
    yew::use_context::<provider::NostrIdStore>()
}

#[yew::hook]
pub fn use_nostr_key() -> Option<nostro2_signer::NostrKeypair> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    key_ctx.get_nostr_key().cloned()
}

#[yew::hook]
pub fn use_nostr_pubkey() -> Option<String> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    key_ctx.get_pubkey().map(str::to_string)
}

#[yew::hook]
pub fn use_create_local_key() -> Option<yew::callback::Callback<nostro2_signer::NostrKeypair>> {
    use nostro2::NostrSigner as _;
    let idb_ctx = crate::use_idb_database()?;
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    Some(yew::callback::Callback::from(
        move |keypair: nostro2_signer::NostrKeypair| {
            let idb_ctx = idb_ctx.clone();
            let key_ctx = key_ctx.clone();

            yew::platform::spawn_local(async move {
                let entry = match crate::IdbKeypairEntry::from_keypair(keypair.clone()).await {
                    Ok(entry) => entry,
                    Err(e) => {
                        crate::DevConsole::error(format_args!(
                            "nostr-minions: could not prepare the key for storage: {e}"
                        ));
                        return;
                    }
                };
                if let Err(e) = idb_ctx.add_identity(entry).await {
                    crate::DevConsole::error(format_args!(
                        "nostr-minions: could not save the key: {e}"
                    ));
                    return;
                }
                key_ctx.dispatch(provider::NostrIdAction::LoadIdentity(
                    keypair.public_key(),
                    keypair,
                ));
            });
        },
    ))
}

#[yew::hook]
pub fn use_delete_local_key() -> Option<yew::callback::Callback<()>> {
    use nostro2::NostrSigner as _;
    let idb_ctx = crate::use_idb_database()?;
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    Some(yew::callback::Callback::from(move |()| {
        let idb_ctx = idb_ctx.clone();
        let key_dispatch = key_ctx.dispatcher();
        let Some(key) = key_ctx.get_nostr_key().cloned() else {
            return;
        };
        yew::platform::spawn_local(async move {
            if let Err(e) = idb_ctx.remove_identity(key.public_key()).await {
                crate::DevConsole::error(format_args!(
                    "nostr-minions: could not delete the key: {e}"
                ));
                return;
            }
            key_dispatch.dispatch(provider::NostrIdAction::DeleteIdentity);
        });
    }))
}
