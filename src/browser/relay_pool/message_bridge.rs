//! Conversions between the bridge's message types and the transport's.
//!
//! Two vocabularies exist for the same messages, and both must:
//!
//! - [`RelayCommand`] / [`WorkerOut`] are `Serialize`, because the `yew-agent`
//!   bridge moves them as JSON.
//!   [`Inbound`](wire_envelope::Inbound) / [`Outbound`](wire_envelope::Outbound)
//!   are plain data with a flat binary encoding, and stay free of `serde` and
//!   of the web bindings so their tests run on the host.
//!
//! Keeping them separate is what lets the transport be chosen at runtime. These
//! impls are the single seam between them, so the worker and the provider each
//! speak ONE vocabulary and convert at the edge.
//!
//! # The `Inbound` -> `RelayCommand` back-edge
//!
//! `cargo graph` reports this file as a back-edge, because `Inbound` sits
//! deeper in the hierarchy than `RelayCommand` yet appears to point back up.
//! It is an artifact of reading both directions of a `From` pair as edges.
//!
//! There is no real cycle: `wire_envelope` is a leaf crate that does not depend
//! on `nostr-minions` and cannot name `RelayCommand`. Both impls live HERE, on
//! this side of the boundary, which is the only place that knows both types.
//! Do not "fix" it by moving a conversion into `wire_envelope`; that would
//! force the wire crate to take a dependency on the pool and create a genuine
//! cycle.

use crate::transport::{Inbound, Outbound};

use super::websocket::ReadyState;
use super::worker::RelayCommand;

impl From<RelayCommand> for Inbound {
    fn from(command: RelayCommand) -> Self {
        match command {
            RelayCommand::Connect(urls) => Self::Connect(urls),
            RelayCommand::AddRelay(url) => Self::AddRelay(url),
            RelayCommand::RemoveRelay(url) => Self::RemoveRelay(url),
            RelayCommand::Subscribe {
                sub_id,
                filter_json,
                req_json,
            } => Self::Subscribe {
                sub_id,
                filter_json,
                req_json,
            },
            RelayCommand::Close(sub_id) => Self::Close(sub_id),
            RelayCommand::Send(event_json) => Self::Send(event_json),
            #[cfg(feature = "bench-harness")]
            RelayCommand::Flood(_) => Self::Send(String::new()),
        }
    }
}

impl From<Inbound> for RelayCommand {
    fn from(message: Inbound) -> Self {
        match message {
            Inbound::Connect(urls) => Self::Connect(urls),
            Inbound::AddRelay(url) => Self::AddRelay(url),
            Inbound::RemoveRelay(url) => Self::RemoveRelay(url),
            Inbound::Subscribe {
                sub_id,
                filter_json,
                req_json,
            } => Self::Subscribe {
                sub_id,
                filter_json,
                req_json,
            },
            Inbound::Close(sub_id) => Self::Close(sub_id),
            Inbound::Send(event_json) => Self::Send(event_json),
        }
    }
}

/// Health updates convert freely; note and relay-event frames do NOT.
///
/// [`WorkerOut::Note`] holds the relay's raw string, while
/// [`Outbound::Note`](wire_envelope::Outbound::Note) holds a parsed note.
/// Converting between them would re-parse or re-encode on the worker — the
/// exact cost this transport exists to remove. The worker instead keeps both
/// forms from the moment it parses (see [`super::ingested::Ingested`]) and
/// picks one at the edge.
impl RelayHealth {
    #[must_use]
    pub fn to_wire(updates: Vec<(String, ReadyState)>) -> Outbound {
        Outbound::RelayHealth(
            updates
                .into_iter()
                .map(|(url, state)| (url, state as u8))
                .collect(),
        )
    }

    #[must_use]
    pub fn from_wire(updates: Vec<(String, u8)>) -> Vec<(String, ReadyState)> {
        updates
            .into_iter()
            .map(|(url, state)| (url, ReadyState::from_wire(state)))
            .collect()
    }
}

/// Namespace for the health-update conversions.
pub struct RelayHealth;

impl ReadyState {
    /// Rebuild a state from the `u8` the flat encoding carries.
    ///
    /// The wire crate stays free of this enum, so the mapping lives here.
    pub(crate) const fn from_wire(state: u8) -> Self {
        match state {
            0 => Self::CONNECTING,
            1 => Self::OPEN,
            2 => Self::CLOSING,
            _ => Self::CLOSED,
        }
    }
}
