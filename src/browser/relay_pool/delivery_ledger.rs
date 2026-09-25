use std::collections::HashMap;

use nostro2::NostrNote;

use super::{BoundedDedup, SubscriptionId};

const PER_SUBSCRIPTION_CAPACITY: usize = 10_000;

/// Which notes each open subscription has already been handed.
#[derive(Debug, Default)]
pub struct DeliveryLedger {
    delivered: HashMap<SubscriptionId, BoundedDedup>,
}

impl DeliveryLedger {
    pub fn open(&mut self, id: SubscriptionId) {
        self.delivered
            .insert(id, BoundedDedup::new(PER_SUBSCRIPTION_CAPACITY));
    }

    pub fn close(&mut self, id: &SubscriptionId) {
        self.delivered.remove(id);
    }

    /// Records the delivery and reports whether it is the first for this
    /// subscription. Notes without an id are always delivered.
    #[must_use]
    pub fn first_delivery(&mut self, id: &SubscriptionId, note: &NostrNote) -> bool {
        let Some(note_id) = note.id.as_ref() else {
            return true;
        };
        self.delivered
            .get_mut(id)
            .is_none_or(|seen| seen.insert(note_id.clone()))
    }
}
