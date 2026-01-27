use nostr_minions::{use_notes_by_kind, use_text_notes, NostrAppProvider};
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
            <div class="min-h-screen bg-gray-100 p-8">
                <h1 class="text-3xl font-bold mb-8 text-center">
                    {"Event Stream Test - Two Independent Subscriptions"}
                </h1>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                    <TextNotesComponent />
                    <ReactionsComponent />
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
    let render_count = use_mut_ref(|| 0);
    let message_count = use_mut_ref(|| 0);

    // Track messages received
    let current_notes_len = notes.len();
    if current_notes_len > *message_count.borrow() {
        *message_count.borrow_mut() = current_notes_len;
    }

    // Increment render count
    *render_count.borrow_mut() += 1;
    let count = *render_count.borrow();
    let msg_count = *message_count.borrow();

    web_sys::console::log_1(
        &format!(
            "TextNotesComponent rendered {} times, {} messages received",
            count, msg_count
        )
        .into(),
    );

    html! {
        <div class="bg-white rounded-lg shadow-lg p-6">
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
                    {"📝 Subscribed to: "}
                    <code class="bg-blue-100 px-2 py-1 rounded">{"kind: 1"}</code>
                </p>
                <p class="text-xs text-gray-600 mt-1">
                    {"This component only re-renders when kind 1 notes arrive"}
                </p>
            </div>

            <div class="space-y-3 max-h-96 overflow-y-auto">
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
    let render_count = use_mut_ref(|| 0);
    let message_count = use_mut_ref(|| 0);

    // Track messages received
    let current_reactions_len = reactions.len();
    if current_reactions_len > *message_count.borrow() {
        *message_count.borrow_mut() = current_reactions_len;
    }

    // Increment render count
    *render_count.borrow_mut() += 1;
    let count = *render_count.borrow();
    let msg_count = *message_count.borrow();

    web_sys::console::log_1(
        &format!(
            "ReactionsComponent rendered {} times, {} messages received",
            count, msg_count
        )
        .into(),
    );

    html! {
        <div class="bg-white rounded-lg shadow-lg p-6">
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
                    {"❤️ Subscribed to: "}
                    <code class="bg-purple-100 px-2 py-1 rounded">{"kind: 7"}</code>
                </p>
                <p class="text-xs text-gray-600 mt-1">
                    {"This component only re-renders when kind 7 notes arrive"}
                </p>
            </div>

            <div class="space-y-3 max-h-96 overflow-y-auto">
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
