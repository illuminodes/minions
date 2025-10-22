mod nostr_relay;
mod provider;
mod websocket;
pub use nostr_relay::*;
use nostro2::{NostrClientEvent, NostrNote};
use nostro2_signer::nostro2::NostrSigner;
pub use provider::*;
pub use websocket::*;

#[yew::hook]
pub fn use_nostr_relay_pool() -> provider::NostrRelayPoolStore {
    yew::use_context::<provider::NostrRelayPoolStore>().expect("No Nostr Relay Pool context found")
}

#[yew::function_component(RelayPoolTest)]
pub fn relay_pool_test() -> yew::Html {
    let relay_ctx =
        yew::use_context::<provider::NostrRelayPoolStore>().expect("No relay context found");
    let subscription_id = yew::use_state(|| None);
    let latest_note = yew::use_state(|| None);

    let id_handle = subscription_id.clone();
    let relay_clone = relay_ctx.clone();
    yew::use_effect_with((), move |()| {
        let nostr_sub: NostrClientEvent = nostro2::NostrSubscription {
            kinds: Some(vec![20001]),
            ..Default::default()
        }
        .into();
        let kind_one_filter = nostro2::NostrSubscription {
            kinds: Some(vec![1]),
            limit: Some(20),
            ..Default::default()
        };
        if let nostro2::NostrClientEvent::Subscribe(.., id, _sub) = &nostr_sub {
            let sub_id = id.clone();
            id_handle.set(Some(sub_id));
            relay_clone.send(nostr_sub);
        }
        relay_clone.send(kind_one_filter);
        || {}
    });
    let note_handle = latest_note.clone();
    let note_counter = yew::use_state(|| 0);

    let counter_handle = note_counter.clone();
    yew::use_effect_with(relay_ctx.last_note.clone(), move |last_note| {
        let mut counter = *counter_handle;
        counter += 1;
        counter_handle.set(counter);
        note_handle.set(last_note.clone());
    });

    let note_sender = relay_ctx.clone();
    let send_note_onclick = yew::Callback::from(move |_| {
        web_sys::console::log_1(&"Sending note".into());
        let new_keys = nostro2_signer::keypair::NostrKeypair::generate(false);
        let mut new_note = NostrNote {
            content: "Minion Note".to_string(),
            kind: 20001,
            pubkey: new_keys.public_key(),
            ..Default::default()
        };
        if new_keys.sign_nostr_note(&mut new_note).is_ok() {
            note_sender.send(new_note);
        }
        web_sys::console::log_1(&"Sent note".into());
    });

    subscription_id.as_ref().map_or_else(
        || yew::html! { <div>{"Loading Relay Pool..."}</div> },
        |id| {
            let unsubscriber = relay_ctx;
            let sub_id = nostro2::NostrClientEvent::close_subscription(id);
            let unsubscribe_onclick = yew::Callback::from(move |_| {
                unsubscriber.send(sub_id.clone());
            });
            yew::html! {
                <div class="flex flex-col gap-4">
                    <h2 class="text-xl font-bold">{"Relay Pool Test"}</h2>
                    <div class="flex flex-row gap-2">
                        <button onclick={send_note_onclick}>
                            { "Send Note" }
                        </button>
                        <button onclick={unsubscribe_onclick}>
                            { "Unsubscribe" }
                        </button>
                    </div>
                    <div>
                        <h3>{"Kind 1 Count"}</h3>
                        <p>{*note_counter}</p>
                    </div>
                    {latest_note.as_ref().map_or_else(
                        ||
                        yew::html! { <div>{"Send a note!"}</div> },
                        |note|
                        yew::html! {
                            <div>
                                <h3>{"My Latest Note"}</h3>
                                <p>{note.content.as_str()}</p>
                            </div>
                    })}
                </div>
            }
        },
    )
}
