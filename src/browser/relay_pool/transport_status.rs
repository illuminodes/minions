//! Which transport the pool settled on, as observable state.
//!
//! The choice is automatic and correct either way, so nothing in the pool needs
//! to ask. It is reported because a SILENT fallback and a working ring path
//! look identical from the outside: notes arrive in both cases.
//!
//! Without this, a page that lost its isolation headers would keep working, at
//! bridge cost, and no test could tell. That is the failure this type makes
//! visible.

/// The transport carrying pool traffic.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TransportStatus {
    /// The worker has not offered rings yet.
    ///
    /// Every page starts here. It is not an error: commands sent now travel the
    /// bridge, which is connected from the first frame.
    #[default]
    Pending,
    /// The JSON bridge carries both directions.
    ///
    /// The page is not cross-origin isolated, or the build has no
    /// `sab-transport` feature.
    Bridge,
    /// The shared rings carry both directions.
    SharedRings,
}

impl TransportStatus {
    /// Whether the shared rings won.
    #[must_use]
    pub const fn is_shared_rings(self) -> bool {
        matches!(self, Self::SharedRings)
    }

    /// Whether the choice is final.
    ///
    /// The pool decides once, so a settled status never changes again.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        !matches!(self, Self::Pending)
    }

    /// A short label for a UI or a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "negotiating",
            Self::Bridge => "JSON bridge",
            Self::SharedRings => "shared rings",
        }
    }
}

impl std::fmt::Display for TransportStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::TransportStatus;

    #[test]
    fn a_fresh_pool_has_not_chosen_yet() {
        assert_eq!(TransportStatus::default(), TransportStatus::Pending);
        assert!(!TransportStatus::default().is_settled());
    }

    #[test]
    fn only_shared_rings_counts_as_shared_rings() {
        assert!(TransportStatus::SharedRings.is_shared_rings());
        assert!(!TransportStatus::Bridge.is_shared_rings());
        assert!(!TransportStatus::Pending.is_shared_rings());
    }

    #[test]
    fn both_outcomes_are_settled() {
        assert!(TransportStatus::Bridge.is_settled());
        assert!(TransportStatus::SharedRings.is_settled());
    }

    #[test]
    fn every_status_has_a_distinct_label() {
        let labels = [
            TransportStatus::Pending.label(),
            TransportStatus::Bridge.label(),
            TransportStatus::SharedRings.label(),
        ];
        let mut unique = labels.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), labels.len());
    }
}
