mod nostr_id;
mod provider;

pub use nostr_id::*;
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
    let keypair =
        yew::suspense::use_future_with(
            key_ctx,
            |key_ctx| async move { key_ctx.get_nostr_key().await },
        )
        .ok()?;
    (*keypair).clone()
}
#[yew::hook]
pub fn use_nostr_id() -> Option<UserIdentity> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    key_ctx.get_identity().cloned()
}

#[yew::hook]
pub fn use_nostr_pubkey() -> Option<String> {
    let key_ctx = yew::use_context::<provider::NostrIdStore>()?;
    key_ctx.get_pubkey()
}
