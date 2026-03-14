use nostr_minions::{use_notes_by_kind, use_relay_events, use_text_notes, NostrAppProvider};
use yew::prelude::*;

#[wasm_bindgen_test::wasm_bindgen_test]
pub fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    let relays = vec![
        nostr_minions::UserRelay {
            url: "wss://relay.damus.io".to_string(),
            read: true,
            write: true,
        },
        nostr_minions::UserRelay {
            url: "wss://relay.nostr.band".to_string(),
            read: true,
            write: true,
        },
    ];

    html! {
        <NostrAppProvider {relays} fallback={html!(<Splash/>)}>
            <div style="height: 100vh; display: flex; flex-direction: column; overflow: hidden; padding: 1rem; background: #f3f4f6;">
                <h1 class="text-2xl font-bold mb-4 text-center" style="flex-shrink: 0;">
                    {"Event Stream Test"}
                </h1>

                <div style="display: flex; gap: 1rem; flex: 1; min-height: 0;">
                    <div style="flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden;">
                        <TextNotesComponent />
                    </div>
                    <div style="flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden;">
                        <ReactionsComponent />
                    </div>
                    <div style="flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden;">
                        <RelayEventsComponent />
                    </div>
                </div>
            </div>
        </NostrAppProvider>
    }
}

#[function_component(Splash)]
fn splash() -> Html {
    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center">
            <div class="bg-white p-8 rounded-lg shadow-xl">
                <p class="text-xl">{"Loading..."}</p>
            </div>
        </div>
    }
}

/// Component subscribing to kind 1 (text notes)
/// Should only re-render when kind 1 notes arrive
#[function_component(TextNotesComponent)]
fn text_notes_component() -> Html {
    let notes = use_text_notes(Some(20));
    let render_count = use_mut_ref(|| 0usize);

    // Increment render count
    *render_count.borrow_mut() += 1;
    let count = *render_count.borrow();
    let msg_count = notes.len();

    web_sys::console::log_1(
        &format!(
            "TextNotesComponent rendered {} times, {} notes in buffer",
            count, msg_count
        )
        .into(),
    );

    html! {
        <div class="bg-white rounded-lg shadow-lg" style="padding: 1rem; display: flex; flex-direction: column; overflow: hidden; flex: 1;">
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-2xl font-bold text-blue-600">
                    {"Text Notes (Kind 1)"}
                </h2>
                <div class="text-right">
                    <div class="text-sm text-gray-500">
                        {format!("Renders: {}", count)}
                    </div>
                    <div class="text-xs text-blue-600 font-semibold">
                        {format!("Messages: {}", msg_count)}
                    </div>
                </div>
            </div>

            <div class="mb-4 p-3 bg-blue-50 rounded">
                <p class="text-sm text-gray-700">
                    {"Subscribed to: "}
                    <code class="bg-blue-100 px-2 py-1 rounded">{"kind: 1"}</code>
                </p>
                <p class="text-xs text-gray-600 mt-1">
                    {"This component only re-renders when kind 1 notes arrive"}
                </p>
            </div>

            <div class="space-y-3" style="flex: 1; min-height: 0; overflow-y: auto;">
                {if notes.is_empty() {
                    html! {
                        <p class="text-gray-500 italic text-center py-4">
                            {"Waiting for text notes..."}
                        </p>
                    }
                } else {
                    html! {
                        <>
                            <p class="text-sm font-semibold text-gray-700 mb-2">
                                {format!("Received {} notes:", notes.len())}
                            </p>
                            {for notes.iter().map(|note| {
                                html! {
                                    <div class="p-3 bg-gray-50 rounded border border-gray-200">
                                        <div class="flex justify-between text-xs text-gray-500 mb-2">
                                            <span class="font-mono truncate max-w-xs">
                                                {format!("From: {}...", &note.pubkey[..16])}
                                            </span>
                                            <span>
                                                {format_timestamp(note.created_at)}
                                            </span>
                                        </div>
                                        <p class="text-sm text-gray-800 line-clamp-3">
                                            {&note.content}
                                        </p>
                                    </div>
                                }
                            })}
                        </>
                    }
                }}
            </div>
        </div>
    }
}

/// Component subscribing to kind 7 (reactions)
/// Should only re-render when kind 7 notes arrive
#[function_component(ReactionsComponent)]
fn reactions_component() -> Html {
    let reactions = use_notes_by_kind(7, Some(20));
    let render_count = use_mut_ref(|| 0usize);

    // Increment render count
    *render_count.borrow_mut() += 1;
    let count = *render_count.borrow();
    let msg_count = reactions.len();

    web_sys::console::log_1(
        &format!(
            "ReactionsComponent rendered {} times, {} reactions in buffer",
            count, msg_count
        )
        .into(),
    );

    html! {
        <div class="bg-white rounded-lg shadow-lg" style="padding: 1rem; display: flex; flex-direction: column; overflow: hidden; flex: 1;">
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-2xl font-bold text-purple-600">
                    {"Reactions (Kind 7)"}
                </h2>
                <div class="text-right">
                    <div class="text-sm text-gray-500">
                        {format!("Renders: {}", count)}
                    </div>
                    <div class="text-xs text-purple-600 font-semibold">
                        {format!("Messages: {}", msg_count)}
                    </div>
                </div>
            </div>

            <div class="mb-4 p-3 bg-purple-50 rounded">
                <p class="text-sm text-gray-700">
                    {"Subscribed to: "}
                    <code class="bg-purple-100 px-2 py-1 rounded">{"kind: 7"}</code>
                </p>
                <p class="text-xs text-gray-600 mt-1">
                    {"This component only re-renders when kind 7 notes arrive"}
                </p>
            </div>

            <div class="space-y-3" style="flex: 1; min-height: 0; overflow-y: auto;">
                {if reactions.is_empty() {
                    html! {
                        <p class="text-gray-500 italic text-center py-4">
                            {"Waiting for reactions..."}
                        </p>
                    }
                } else {
                    html! {
                        <>
                            <p class="text-sm font-semibold text-gray-700 mb-2">
                                {format!("Received {} reactions:", reactions.len())}
                            </p>
                            {for reactions.iter().map(|note| {
                                html! {
                                    <div class="p-3 bg-gray-50 rounded border border-gray-200">
                                        <div class="flex justify-between text-xs text-gray-500 mb-2">
                                            <span class="font-mono truncate max-w-xs">
                                                {format!("From: {}...", &note.pubkey[..16])}
                                            </span>
                                            <span>
                                                {format_timestamp(note.created_at)}
                                            </span>
                                        </div>
                                        <p class="text-2xl">
                                            {&note.content}
                                        </p>
                                    </div>
                                }
                            })}
                        </>
                    }
                }}
            </div>
        </div>
    }
}

/// Component subscribing to ALL relay events (EOSE, OK, NOTICE, AUTH, notes, etc.)
/// Displays a live feed of raw protocol events from connected relays.
#[function_component(RelayEventsComponent)]
fn relay_events_component() -> Html {
    let events = use_mut_ref(Vec::<String>::new);
    let force_update = use_force_update();

    let eose_count = use_mut_ref(|| 0usize);
    let ok_count = use_mut_ref(|| 0usize);
    let notice_count = use_mut_ref(|| 0usize);
    let other_count = use_mut_ref(|| 0usize);

    {
        let events = events.clone();
        let force_update = force_update.clone();
        let eose_count = eose_count.clone();
        let ok_count = ok_count.clone();
        let notice_count = notice_count.clone();
        let other_count = other_count.clone();

        use_relay_events(Callback::from(
            move |event: nostr_minions::NostrRelayEvent| {
                let label = match &event {
                    nostr_minions::NostrRelayEvent::EndOfSubscription(_, sub_id) => {
                        *eose_count.borrow_mut() += 1;
                        format!("[EOSE] sub={sub_id}")
                    }
                    nostr_minions::NostrRelayEvent::SentOk(_, event_id, success, msg) => {
                        *ok_count.borrow_mut() += 1;
                        format!("[OK] id={event_id} success={success} msg={msg}")
                    }
                    nostr_minions::NostrRelayEvent::Notice(_, msg) => {
                        *notice_count.borrow_mut() += 1;
                        format!("[NOTICE] {msg}")
                    }
                    nostr_minions::NostrRelayEvent::Auth(_, challenge) => {
                        *other_count.borrow_mut() += 1;
                        format!("[AUTH] challenge={challenge}")
                    }
                    nostr_minions::NostrRelayEvent::ClosedSubscription(_, sub_id) => {
                        *other_count.borrow_mut() += 1;
                        format!("[CLOSED] sub={sub_id}")
                    }
                    nostr_minions::NostrRelayEvent::Ping => {
                        *other_count.borrow_mut() += 1;
                        "[PING]".to_string()
                    }
                    nostr_minions::NostrRelayEvent::Close(reason) => {
                        *other_count.borrow_mut() += 1;
                        format!("[CLOSE] {reason}")
                    }
                    // Notes are handled by note subscriptions, not relay event subscribers
                    nostr_minions::NostrRelayEvent::NewNote(..) => return,
                };

                let mut ev = events.borrow_mut();
                ev.insert(0, label);
                ev.truncate(50);
                drop(ev);

                force_update.force_update();
            },
        ));
    }

    let events_snapshot = events.borrow().clone();
    let eose = *eose_count.borrow();
    let ok = *ok_count.borrow();
    let notice = *notice_count.borrow();
    let other = *other_count.borrow();

    html! {
        <div class="bg-white rounded-lg shadow-lg" style="padding: 1rem; display: flex; flex-direction: column; overflow: hidden; flex: 1;">
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-2xl font-bold text-green-600">
                    {"Relay Events"}
                </h2>
            </div>

            <div class="mb-4 p-3 bg-green-50 rounded">
                <p class="text-sm text-gray-700">
                    {"Subscribed to: "}
                    <code class="bg-green-100 px-2 py-1 rounded">{"all relay events"}</code>
                </p>
                <div class="grid grid-cols-4 gap-2 mt-2 text-xs">
                    <div class="text-center">
                        <div class="font-bold text-blue-700">{eose}</div>
                        <div class="text-gray-500">{"EOSE"}</div>
                    </div>
                    <div class="text-center">
                        <div class="font-bold text-yellow-700">{ok}</div>
                        <div class="text-gray-500">{"OK"}</div>
                    </div>
                    <div class="text-center">
                        <div class="font-bold text-red-700">{notice}</div>
                        <div class="text-gray-500">{"NOTICE"}</div>
                    </div>
                    <div class="text-center">
                        <div class="font-bold text-gray-700">{other}</div>
                        <div class="text-gray-500">{"OTHER"}</div>
                    </div>
                </div>
            </div>

            <div class="space-y-1 font-mono text-xs" style="flex: 1; min-height: 0; overflow-y: auto;">
                {if events_snapshot.is_empty() {
                    html! {
                        <p class="text-gray-500 italic text-center py-4">
                            {"Waiting for relay events..."}
                        </p>
                    }
                } else {
                    html! {
                        <>
                            {for events_snapshot.iter().map(|ev| {
                                let color = if ev.starts_with("[NOTE]") {
                                    "text-green-700"
                                } else if ev.starts_with("[EOSE]") {
                                    "text-blue-700"
                                } else if ev.starts_with("[OK]") {
                                    "text-yellow-700"
                                } else if ev.starts_with("[NOTICE]") {
                                    "text-red-700"
                                } else {
                                    "text-gray-600"
                                };
                                html! {
                                    <div class={classes!("p-1", "bg-gray-50", "rounded", "truncate", color)}>
                                        {ev}
                                    </div>
                                }
                            })}
                        </>
                    }
                }}
            </div>
        </div>
    }
}

/// Format Unix timestamp to relative time
fn format_timestamp(timestamp: i64) -> String {
    let now = nostr_minions::NostrNote::now();
    let diff = now - timestamp;

    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}
