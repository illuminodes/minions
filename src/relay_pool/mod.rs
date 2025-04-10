mod nostr_relay;
mod provider;
pub use nostr_relay::*;
use nostro2_web_relay::nostro2::{note::NostrNote, relay_events::NostrClientEvent};
pub use provider::*;

#[yew::function_component(RelayPoolTest)]
pub fn relay_pool_test() -> yew::Html {
    let relay_ctx =
        yew::use_context::<provider::NostrRelayPoolStore>().expect("No relay context found");
    let subscription_id = yew::use_state(|| None);
    let latest_note = yew::use_state(|| None);

    let id_handle = subscription_id.clone();
    let relay_clone = relay_ctx.clone();
    yew::use_effect_with((), move |()| {
        let nostr_sub: NostrClientEvent =
            nostro2_web_relay::nostro2::subscriptions::NostrSubscription {
                kinds: Some(vec![20001]),
                ..Default::default()
            }
            .into();
        let kind_one_filter = nostro2_web_relay::nostro2::subscriptions::NostrSubscription {
            kinds: Some(vec![1]),
            limit: Some(20),
            ..Default::default()
        };
        if let nostro2_web_relay::nostro2::relay_events::NostrClientEvent::Subscribe(.., id, _sub) =
            &nostr_sub
        {
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
    yew::use_effect_with(relay_ctx.unique_notes.clone(), move |relay_clone| {
        gloo::console::log!("Unique notes:", relay_clone.len());
        if let Some(last_note) = relay_clone.last().cloned() {
            let mut counter = *counter_handle;
            counter += 1;
            counter_handle.set(counter);
            note_handle.set(Some(last_note));
        }
        || {}
    });

    let note_sender = relay_ctx.clone();
    let send_note_onclick = yew::Callback::from(move |_| {
        let new_keys = nostro2_signer::keypair::NostrKeypair::generate(false);
        let mut new_note = NostrNote {
            content: "Minion Note".to_string(),
            kind: 20001,
            pubkey: new_keys.public_key().to_string(),
            ..Default::default()
        };
        new_keys.sign_nostr_event(&mut new_note);
        note_sender.send(new_note);
    });

    match subscription_id.as_ref() {
        Some(id) => {
            let unsubscriber = relay_ctx.clone();
            let sub_id = crate::nostro2::relay_events::NostrClientEvent::CloseSubscriptionEvent(
                crate::nostro2::relay_events::RelayEventTag::CLOSE,
                id.to_string(),
            );
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
                    {{
                        match latest_note.as_ref() {
                            Some(note) => yew::html! {
                                <div>
                                    <h3>{"My Latest Note"}</h3>
                                    <p>{note.content.as_str()}</p>
                                </div>
                            },
                            None => yew::html! { <div>{"Send a note!"}</div> },
                        }
                    }}
                </div>
            }
        }
        None => yew::html! { <div>{"Loading Relay Pool..."}</div> },
    }
}
