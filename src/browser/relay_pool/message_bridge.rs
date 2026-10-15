//! Conversions between the bridge's message types and the transport's.
//!
//! Two vocabularies exist for the same messages, and both must:
//!
//! - [`RelayCommand`] / [`WorkerOut`] are `Serialize`, because the `yew-agent`
//!   bridge moves them as JSON.
//! - [`Inbound`] / [`Outbound`] are plain data with a flat binary encoding, and
//!   stay free of `serde` and of the web bindings so their tests run on the
//!   host.
//!
//! Keeping them separate is what lets the transport be chosen at runtime. The
//! conversions here are the single seam between them, so the worker and the
//! provider each speak ONE vocabulary and convert at the edge.
//!
//! The conversions are methods on the pool types, never impls on the wire
//! types, so `transport` never names anything in `browser`.

use crate::transport::{Inbound, Outbound};

use super::websocket::ReadyState;
use super::worker::RelayCommand;

impl RelayCommand {
    #[must_use]
    pub fn to_wire(self) -> Inbound {
        match self {
            Self::Connect(urls) => Inbound::Connect(urls),
            Self::AddRelay(url) => Inbound::AddRelay(url),
            Self::RemoveRelay(url) => Inbound::RemoveRelay(url),
            Self::Subscribe {
                sub_id,
                filter_json,
                req_json,
            } => Inbound::Subscribe {
                sub_id,
                filter_json,
                req_json,
            },
            Self::Close(sub_id) => Inbound::Close(sub_id),
            Self::Send(event_json) => Inbound::Send(event_json),
            #[cfg(feature = "bench-harness")]
            Self::Flood(_) => Inbound::Send(String::new()),
        }
    }

    #[must_use]
    pub fn from_wire(message: Inbound) -> Self {
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
/// [`Outbound::Note`] holds a parsed note.
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
