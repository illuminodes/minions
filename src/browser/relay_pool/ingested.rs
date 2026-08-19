//! What the ingestor decides a relay frame is worth.
//!
//! # Why this is not just a message
//!
//! The two transports want different things from the same frame:
//!
//! - The **bridge** wants the relay's ORIGINAL string. The worker already
//!   parsed it, so re-encoding would be waste.
//! - The **rings** want the PARSED note, so the flat encoding can be written
//!   straight out and the UI thread never runs a JSON parse.
//!
//! The ingestor has both at the moment it decides, and throwing either away
//! forces the loser to redo work. So it returns both, and each transport takes
//! what it needs at the edge.
//!
//! That the raw string was ever the only output is exactly what the benchmark
//! measured: forwarding it moved the parse onto the UI thread and cost 11% of
//! animation frames.

use nostro2::NostrNote;

use super::worker::WorkerOut;

/// A frame the worker decided to ship.
#[derive(Clone, Debug)]
pub enum Ingested {
    /// A deduped note matching an active filter, kept in both forms.
    Note {
        /// The relay's own EVENT frame.
        raw: String,
        /// The same note, already parsed.
        note: Box<NostrNote>,
    },
    /// A non-note relay frame (EOSE / OK / NOTICE / AUTH / CLOSED).
    RelayEvent(String),
}

impl Ingested {
    /// The bridge form, which carries relay frames as their original strings.
    #[must_use]
    pub fn into_bridge(self) -> WorkerOut {
        match self {
            Self::Note { raw, .. } => WorkerOut::Note(raw),
            Self::RelayEvent(raw) => WorkerOut::RelayEvent(raw),
        }
    }

    /// The shared-ring form, which carries notes already parsed.
    #[must_use]
    pub fn into_wire(self) -> crate::transport::Outbound {
        match self {
            Self::Note { note, .. } => crate::transport::Outbound::Note(*note),
            Self::RelayEvent(raw) => crate::transport::Outbound::RelayEvent(raw),
        }
    }
}
