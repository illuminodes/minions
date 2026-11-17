//! Relay frame ingestion: parse → dedup → filter-match → verify.
//!
//! This is the hot path every incoming relay frame takes, extracted from the
//! socket callback that used to own it so that exactly ONE implementation
//! exists. The reactor's sockets call it, and `worker_bench` drives the same
//! type through the same reactor — the benchmark cannot drift from the shipped
//! behaviour, because there is nothing separate left to drift from.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use nostro2::{NostrClientEvent, NostrNote, NostrRelayEvent, NostrSubscription};

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
    /// duplicate, a note whose id or signature does not verify, or a note no
    /// active filter wants.
    ///
    /// A relay is not trusted to vouch for a note. Without the check it could
    /// deliver a note under any author's pubkey, or occupy a real note's id in
    /// the dedup tracker so the genuine copy from another relay is dropped.
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
        self.ingest_frame(raw, true)
    }

    /// [`Self::ingest`] without the signature check, for synthetic frames.
    ///
    /// `worker_bench` floods the reactor with generated notes that carry no
    /// real signature, so they can only travel this path.
    #[cfg(feature = "bench-harness")]
    #[must_use]
    pub fn ingest_unverified(&self, raw: String) -> Option<Ingested> {
        self.ingest_frame(raw, false)
    }

    fn ingest_frame(&self, raw: String, verify: bool) -> Option<Ingested> {
        let event = raw.parse::<NostrRelayEvent>().ok()?;
        let NostrRelayEvent::NewNote(_, sub_id, note) = event else {
            return Some(Ingested::RelayEvent(raw));
        };
        self.accept(&sub_id, &note, verify).then(|| Ingested::Note {
            raw,
            note: Box::new(note),
        })
    }

    /// Decide whether a note is shipped, and record it if so.
    ///
    /// Cheapest checks first: a copy already delivered by another relay, or a
    /// note no filter wants, costs no signature check. The note is recorded
    /// only after it verifies, so a forged copy cannot shadow the genuine one.
    fn accept(&self, sub_id: &str, note: &NostrNote, verify: bool) -> bool {
        let dedup_key = note.id.as_ref().map(|id| format!("{sub_id}:{id}"));
        if self.seen(dedup_key.as_deref()) || !self.wanted(note) || !Self::genuine(note, verify) {
            return false;
        }
        self.record(dedup_key);
        true
    }

    /// Remember a shipped note, so a later copy is not shipped again.
    fn record(&self, dedup_key: Option<String>) {
        if let Some(key) = dedup_key {
            self.dedup.borrow_mut().insert(key);
        }
    }

    /// Whether a note with this dedup key was already shipped.
    fn seen(&self, dedup_key: Option<&str>) -> bool {
        dedup_key.is_some_and(|key| self.dedup.borrow().contains(key))
    }

    /// Whether any active filter matches the note.
    fn wanted(&self, note: &NostrNote) -> bool {
        self.filters.borrow().values().any(|f| f.matches(note))
    }

    /// Whether the note's id and signature verify, when verification is on.
    fn genuine(note: &NostrNote, verify: bool) -> bool {
        use nostro2::NostrEvent as _;
        !verify || note.verify()
    }
}
