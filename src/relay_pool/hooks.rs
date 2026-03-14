use nostro2::{NostrNote, NostrSubscription};
use yew::prelude::*;

use super::SubscriptionId;

/// Subscribe to notes matching a filter
///
/// Returns a vector of notes that match the filter. The hook automatically
/// manages subscription lifecycle (subscribe on mount, unsubscribe on unmount).
///
/// Returns an empty vec if the relay pool context is not available.
///
/// # Example
/// ```rust
/// let notes = use_nostr_notes(NostrSubscription {
///     kinds: Some(vec![1]),
///     limit: Some(50),
///     ..Default::default()
/// });
/// ```
#[hook]
pub fn use_nostr_notes(filter: NostrSubscription) -> Vec<NostrNote> {
    let pool = super::use_nostr_relay_pool();
    let notes = use_mut_ref(Vec::<NostrNote>::new);
    let force_update = use_force_update();
    let sub_id: std::rc::Rc<std::cell::RefCell<Option<SubscriptionId>>> = use_mut_ref(|| None);

    let Some(pool) = pool else {
        return Vec::new();
    };

    // Subscribe on mount or filter change
    #[allow(clippy::redundant_clone)]
    use_effect_with(filter.clone(), {
        let pool = pool.clone();
        let notes = notes.clone();
        let force_update = force_update.clone();
        let sub_id = sub_id.clone();

        move |filter| {
            // Unsubscribe from previous if exists
            if let Some(id) = sub_id.borrow_mut().take() {
                pool.unsubscribe(&id);
            }

            // Subscribe with callback
            let id = pool.subscribe(filter.clone(), {
                let limit = filter.limit;

                Callback::from(move |note: NostrNote| {
                    {
                        let mut current = notes.borrow_mut();
                        current.insert(0, note);

                        // Respect limit from filter
                        if let Some(limit) = limit {
                            current.truncate(limit as usize);
                        }
                    }
                    force_update.force_update();
                })
            });

            *sub_id.borrow_mut() = Some(id);

            // Cleanup on unmount
            move || {
                if let Some(id) = sub_id.borrow_mut().take() {
                    pool.unsubscribe(&id);
                }
            }
        }
    });

    let result = notes.borrow().clone();
    result
}

/// Subscribe to relay events (EOSE, OK, NOTICE, AUTH, etc.)
///
/// Note events are excluded — use `use_nostr_notes` for those.
/// Components can pattern match on the event type to handle specific events.
///
/// # Example
/// ```rust
/// use_relay_events(Callback::from(|event: NostrRelayEvent| {
///     match event {
///         NostrRelayEvent::EndOfSubscription(..) => { /* handle EOSE */ }
///         NostrRelayEvent::Notice(.., msg) => { /* handle notice */ }
///         NostrRelayEvent::Auth(.., challenge) => { /* handle auth */ }
///         NostrRelayEvent::SentOk(..) => { /* handle OK */ }
///         _ => {}
///     }
/// }));
/// ```
#[hook]
pub fn use_relay_events(callback: Callback<nostro2::NostrRelayEvent>) {
    let pool = super::use_nostr_relay_pool();
    let sub_id: std::rc::Rc<std::cell::RefCell<Option<SubscriptionId>>> = use_mut_ref(|| None);

    let Some(pool) = pool else {
        return;
    };

    use_effect_with(callback, {
        move |callback| {
            // Unsubscribe from previous if exists
            if let Some(id) = sub_id.borrow_mut().take() {
                pool.unsubscribe_relay_events(&id);
            }

            let id = pool.subscribe_relay_events(callback.clone());
            *sub_id.borrow_mut() = Some(id);

            // Cleanup on unmount
            move || {
                if let Some(id) = sub_id.borrow_mut().take() {
                    pool.unsubscribe_relay_events(&id);
                }
            }
        }
    });
}

/// Subscribe to text notes (kind 1)
///
/// # Example
/// ```rust
/// let notes = use_text_notes(Some(50));
/// ```
#[hook]
pub fn use_text_notes(limit: Option<u32>) -> Vec<NostrNote> {
    use_nostr_notes(NostrSubscription {
        kinds: Some(vec![1]),
        limit,
        ..Default::default()
    })
}

/// Subscribe to notes from specific authors
///
/// # Example
/// ```rust
/// let notes = use_notes_by_authors(vec!["alice_pk".to_string()], Some(20));
/// ```
#[hook]
pub fn use_notes_by_authors(authors: Vec<String>, limit: Option<u32>) -> Vec<NostrNote> {
    use_nostr_notes(NostrSubscription {
        authors: Some(authors),
        limit,
        ..Default::default()
    })
}

/// Subscribe to notes with specific kind
///
/// # Example
/// ```rust
/// let notes = use_notes_by_kind(7, Some(50));
/// ```
#[hook]
pub fn use_notes_by_kind(kind: u32, limit: Option<u32>) -> Vec<NostrNote> {
    use_nostr_notes(NostrSubscription {
        kinds: Some(vec![kind]),
        limit,
        ..Default::default()
    })
}

/// Subscribe to recent notes (last N seconds)
///
/// The `since` timestamp is recomputed on each render so the window
/// slides forward as time passes.
///
/// # Example
/// ```rust
/// let notes = use_recent_notes(vec![1], 3600, Some(50)); // Last hour
/// ```
#[hook]
pub fn use_recent_notes(kinds: Vec<u32>, seconds: i64, limit: Option<u32>) -> Vec<NostrNote> {
    #[allow(clippy::cast_sign_loss)] // `.max(0)` guarantees non-negative
    let since = (NostrNote::now() - seconds).max(0) as u64;

    use_nostr_notes(NostrSubscription {
        kinds: Some(kinds),
        since: Some(since),
        limit,
        ..Default::default()
    })
}

/// Subscribe to a single most recent note matching filter
///
/// # Example
/// ```rust
/// let note = use_live_note(NostrSubscription {
///     kinds: Some(vec![1]),
///     ..Default::default()
/// });
/// ```
#[hook]
pub fn use_live_note(filter: NostrSubscription) -> Option<NostrNote> {
    let notes = use_nostr_notes(filter);
    notes.first().cloned()
}
