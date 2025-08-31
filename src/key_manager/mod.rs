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
