mod bounded_dedup;
#[cfg(feature = "sab-transport")]
mod endpoint;
#[cfg(feature = "bench-harness")]
mod flood;
mod hooks;
mod ingest;
mod ingested;
mod message_bridge;
#[cfg(feature = "sab-transport")]
mod isolation;
mod nostr_relay;
mod pool_transport;
mod app_link;
mod handshake;
mod pool_event;
mod provider;
mod relay_set;
#[cfg(feature = "sab-transport")]
mod ring_pair;
mod ring_poller;
mod ring_pump;
#[cfg(feature = "sab-transport")]
mod sab_ring;
mod subscription;
mod transport_negotiation;
mod transport_status;
mod websocket;
mod worker;
mod worker_boot;
mod worker_sink;

pub use bounded_dedup::BoundedDedup;
#[cfg(feature = "bench-harness")]
pub use flood::{FloodRunner, FloodSpec, SyntheticFrame};
pub use ingest::NoteIngestor;
pub use worker::{JsonCodec, RelayCommand, RelayReactor, WorkerOut};
pub use worker_boot::{relay_worker_main, spawn_relay_bridge};
pub use hooks::{
    use_live_note, use_nostr_notes, use_notes_by_authors, use_notes_by_kind, use_recent_notes,
    use_relay_events, use_text_notes,
};
pub use nostr_relay::UserRelay;
pub use provider::{
    NostrRelayPool, NostrRelayPoolAction, NostrRelayPoolProvider, NostrRelayPoolStore,
};
pub use subscription::{RelayEventSubscription, SubscriptionId, SubscriptionInfo};
pub use transport_status::TransportStatus;
pub use websocket::ReadyState;
pub use worker_boot::is_relay_worker;

#[yew::hook]
pub fn use_nostr_relay_pool() -> Option<provider::NostrRelayPoolStore> {
    yew::use_context::<provider::NostrRelayPoolStore>()
}

#[cfg(feature = "test-components")]
#[yew::function_component(RelayPoolTest)]
pub fn relay_pool_test() -> yew::Html {
    let notes = use_nostr_notes(nostro2::NostrSubscription::new().kind(1).limit(20));
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
