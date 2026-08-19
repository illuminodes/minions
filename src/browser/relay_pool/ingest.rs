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

use nostro2::{NostrClientEvent, NostrRelayEvent, NostrSubscription};

use super::ingested::Ingested;
use super::BoundedDedup;

/// Owns the dedup tracker and the active filter set, and decides what (if
/// anything) each raw relay frame becomes on the bridge.
///
/// Cheap to clone: both fields are `Rc`, so every socket closure shares one
/// dedup tracker and one filter map.
#[derive(Clone)]
pub struct NoteIngestor {
    dedup: Rc<RefCell<BoundedDedup>>,
    filters: Rc<RefCell<HashMap<String, NostrSubscription>>>,
}

impl NoteIngestor {
    #[must_use]
    pub fn new(dedup_capacity: usize) -> Self {
        Self {
            dedup: Rc::new(RefCell::new(BoundedDedup::new(dedup_capacity))),
            filters: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn insert_filter(&self, sub_id: String, filter: NostrSubscription) {
        self.filters.borrow_mut().insert(sub_id, filter);
    }

    pub fn remove_filter(&self, sub_id: &str) {
        self.filters.borrow_mut().remove(sub_id);
    }

    /// REQ frames for every active filter, used to resume subscriptions on a
    /// reconnected socket.
    #[must_use]
    pub fn active_reqs(&self) -> Vec<NostrClientEvent> {
        self.filters
            .borrow()
            .values()
            .map(|f| f.clone().into())
            .collect()
    }

    /// Ingest one raw relay frame.
    ///
    /// Returns what to ship, or `None` when the frame is unparseable, a
    /// duplicate, or a note no active filter wants.
    ///
    /// A matched note is returned in BOTH forms — the original string and the
    /// parsed note — because the parse happened here and the transport in use
    /// decides which form costs nothing. See [`Ingested`].
    #[must_use]
    pub fn ingest(&self, raw: String) -> Option<Ingested> {
        let Ok(event) = raw.parse::<NostrRelayEvent>() else {
            return None;
        };

        let NostrRelayEvent::NewNote(.., note) = event else {
            return Some(Ingested::RelayEvent(raw));
        };

        if let Some(ref id) = note.id {
            if !self.dedup.borrow_mut().insert(id.clone()) {
                return None;
            }
        }

        self.filters
            .borrow()
            .values()
            .any(|f| f.matches(&note))
            .then(|| Ingested::Note {
                raw,
                note: Box::new(note),
            })
    }
}
