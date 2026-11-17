//! Relay frame ingestion: parse → dedup → filter-match.
//!
//! This is the hot path every incoming relay frame takes, extracted from the
//! socket callback that used to own it so that exactly ONE implementation
//! exists. The reactor's sockets call it, and `worker_bench` drives the same
//! type through the same reactor — the benchmark cannot drift from the shipped
//! behaviour, because there is nothing separate left to drift from.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use nostro2::{NostrRelayEvent, NostrSubscription};

use super::ingested::Ingested;
use super::BoundedDedup;

/// One active subscription: the filter to match against, and the exact REQ
/// frame the relays were sent for it.
struct ActiveFilter {
    filter: NostrSubscription,
    req_json: String,
}

/// Owns the dedup tracker and the active filter set, and decides what (if
/// anything) each raw relay frame becomes on the bridge.
///
/// Cheap to clone: both fields are `Rc`, so every socket closure shares one
/// dedup tracker and one filter map.
#[derive(Clone)]
pub struct NoteIngestor {
    dedup: Rc<RefCell<BoundedDedup>>,
    filters: Rc<RefCell<HashMap<String, ActiveFilter>>>,
}

impl NoteIngestor {
    #[must_use]
    pub fn new(dedup_capacity: usize) -> Self {
        Self {
            dedup: Rc::new(RefCell::new(BoundedDedup::new(dedup_capacity))),
            filters: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Record a subscription, with the REQ frame that opened it.
    ///
    /// The frame is kept so a reconnected socket can resume the subscription
    /// under the SAME id. Rebuilding the REQ from the filter would mint a new
    /// id, and the later CLOSE (which carries the original) would not match.
    pub fn insert_filter(&self, sub_id: String, filter: NostrSubscription, req_json: String) {
        self.filters
            .borrow_mut()
            .insert(sub_id, ActiveFilter { filter, req_json });
    }

    pub fn remove_filter(&self, sub_id: &str) {
        self.filters.borrow_mut().remove(sub_id);
    }

    /// The original REQ frame of every active filter, used to open the
    /// subscriptions on a socket that has just connected or reconnected.
    #[must_use]
    pub fn active_reqs(&self) -> Vec<String> {
        self.filters
            .borrow()
            .values()
            .map(|active| active.req_json.clone())
            .collect()
    }

    /// Ingest one raw relay frame.
    ///
    /// Returns what to ship, or `None` when the frame is unparseable, a
    /// duplicate, or a note no active filter wants.
    ///
    /// Duplicates are tracked per subscription: the same note arriving for a
    /// newly opened subscription is shipped again, so that subscription is not
    /// starved by an earlier one having already received it.
    ///
    /// A matched note is returned in BOTH forms — the original string and the
    /// parsed note — because the parse happened here and the transport in use
    /// decides which form costs nothing. See [`Ingested`].
    #[must_use]
    pub fn ingest(&self, raw: String) -> Option<Ingested> {
        let Ok(event) = raw.parse::<NostrRelayEvent>() else {
            return None;
        };

        let NostrRelayEvent::NewNote(_, sub_id, note) = event else {
            return Some(Ingested::RelayEvent(raw));
        };

        if let Some(ref id) = note.id {
            if !self.dedup.borrow_mut().insert(format!("{sub_id}:{id}")) {
                return None;
            }
        }

        self.filters
            .borrow()
            .values()
            .any(|active| active.filter.matches(&note))
            .then(|| Ingested::Note {
                raw,
                note: Box::new(note),
            })
    }
}
