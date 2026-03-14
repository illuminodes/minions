mod bounded_dedup;
mod hooks;
mod nostr_relay;
mod provider;
mod subscription;
mod websocket;

pub use bounded_dedup::BoundedDedup;
pub use hooks::{
    use_live_note, use_nostr_notes, use_notes_by_authors, use_notes_by_kind, use_recent_notes,
    use_text_notes,
};
pub use nostr_relay::UserRelay;
pub use provider::{
    NostrRelayPool, NostrRelayPoolAction, NostrRelayPoolProvider, NostrRelayPoolStore,
};
pub use subscription::{note_matches_filter, SubscriptionId, SubscriptionInfo};
pub use websocket::NostrWebSocket;
pub use websocket::ReadyState;

#[yew::hook]
pub fn use_nostr_relay_pool() -> provider::NostrRelayPoolStore {
    yew::use_context::<provider::NostrRelayPoolStore>().expect("No Nostr Relay Pool context found")
}

#[cfg(feature = "test-components")]
#[yew::function_component(RelayPoolTest)]
pub fn relay_pool_test() -> yew::Html {
    let notes = use_nostr_notes(nostro2::NostrSubscription {
        kinds: Some(vec![1]),
        limit: Some(20),
        ..Default::default()
    });
    let note_count = notes.len();

    yew::html! {
        <div class="flex flex-col gap-4">
            <h2 class="text-xl font-bold">{"Relay Pool Test"}</h2>
            <div>
                <h3>{"Kind 1 Count"}</h3>
                <p>{note_count}</p>
            </div>
            {notes.first().map_or_else(
                || yew::html! { <div>{"Waiting for notes..."}</div> },
                |note| yew::html! {
                    <div>
                        <h3>{"Latest Note"}</h3>
                        <p>{note.content.as_str()}</p>
                    </div>
                },
            )}
        </div>
    }
}
