//! When the transport choice becomes final, and what it becomes.
//!
//! # Why this is not inline in the driver loop
//!
//! The loop has three wake-up sources (commands, bridge frames, animation
//! frames) and the choice can settle from two of them. Keeping the rule here
//! means the loop asks the same question in every arm and cannot answer it
//! differently in one of them — which is exactly the bug this type replaces,
//! where only the bridge arm could settle the status.
//!
//! # Why a tick budget
//!
//! A page that is NOT cross-origin isolated gets no ring offer, ever. Nothing
//! arrives to prove the absence, so waiting for evidence would sit at
//! [`TransportStatus::Pending`] forever. After [`Self::PATIENCE_TICKS`] frames
//! with a live worker and no offer, the bridge is the answer.

use super::transport_status::TransportStatus;

/// Decides when the pool stops negotiating.
pub struct TransportNegotiation {
    ticks_without_offer: u32,
    settled: bool,
}

impl TransportNegotiation {
    /// Animation frames to wait for a ring offer before choosing the bridge.
    ///
    /// The worker must boot, fetch the wasm, and register before it can offer.
    /// At 60Hz this is about three seconds, which is long enough for a cold
    /// load and short enough that a real fallback is visible quickly.
    pub const PATIENCE_TICKS: u32 = 180;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            ticks_without_offer: 0,
            settled: false,
        }
    }

    /// Whether the choice is already final.
    #[allow(dead_code, reason = "the loop acts on the returned status; tests assert on this")]
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        self.settled
    }

    /// Record that the rings were adopted this turn.
    ///
    /// Returns the status to report, once and only once.
    pub const fn rings_adopted(&mut self) -> Option<TransportStatus> {
        self.settle(TransportStatus::SharedRings)
    }

    /// Record one animation frame in which no offer had arrived.
    ///
    /// Returns [`TransportStatus::Bridge`] on the frame the budget runs out.
    pub const fn tick_without_offer(&mut self) -> Option<TransportStatus> {
        if self.settled {
            return None;
        }
        self.ticks_without_offer = self.ticks_without_offer.saturating_add(1);
        if self.ticks_without_offer < Self::PATIENCE_TICKS {
            return None;
        }
        self.settle(TransportStatus::Bridge)
    }

    /// Record a bridge frame that was not the handshake.
    ///
    /// The worker is talking over the bridge, so the rings were never offered
    /// and there is no reason to keep waiting.
    pub const fn bridge_frame(&mut self) -> Option<TransportStatus> {
        self.settle(TransportStatus::Bridge)
    }

    const fn settle(&mut self, status: TransportStatus) -> Option<TransportStatus> {
        if self.settled {
            return None;
        }
        self.settled = true;
        Some(status)
    }
}

impl Default for TransportNegotiation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{TransportNegotiation, TransportStatus};

    #[test]
    fn a_fresh_negotiation_has_not_settled() {
        assert!(!TransportNegotiation::new().is_settled());
    }

    #[test]
    fn adopting_rings_settles_on_shared_rings() {
        let mut negotiation = TransportNegotiation::new();
        assert_eq!(
            negotiation.rings_adopted(),
            Some(TransportStatus::SharedRings)
        );
        assert!(negotiation.is_settled());
    }

    #[test]
    fn a_settled_choice_is_reported_only_once() {
        let mut negotiation = TransportNegotiation::new();
        negotiation.rings_adopted();
        assert_eq!(negotiation.rings_adopted(), None);
        assert_eq!(negotiation.tick_without_offer(), None);
        assert_eq!(negotiation.bridge_frame(), None);
    }

    #[test]
    fn ticks_below_the_budget_keep_waiting() {
        let mut negotiation = TransportNegotiation::new();
        for _ in 1..TransportNegotiation::PATIENCE_TICKS {
            assert_eq!(negotiation.tick_without_offer(), None);
        }
        assert!(!negotiation.is_settled());
    }

    #[test]
    fn the_budget_running_out_chooses_the_bridge() {
        let mut negotiation = TransportNegotiation::new();
        let mut outcome = None;
        for _ in 0..TransportNegotiation::PATIENCE_TICKS {
            outcome = outcome.or(negotiation.tick_without_offer());
        }
        assert_eq!(outcome, Some(TransportStatus::Bridge));
        assert!(negotiation.is_settled());
    }

    #[test]
    fn an_offer_arriving_late_still_wins_over_the_budget() {
        let mut negotiation = TransportNegotiation::new();
        for _ in 0..TransportNegotiation::PATIENCE_TICKS - 1 {
            negotiation.tick_without_offer();
        }
        assert_eq!(
            negotiation.rings_adopted(),
            Some(TransportStatus::SharedRings)
        );
    }

    #[test]
    fn a_bridge_frame_settles_immediately() {
        let mut negotiation = TransportNegotiation::new();
        assert_eq!(negotiation.bridge_frame(), Some(TransportStatus::Bridge));
    }
}
