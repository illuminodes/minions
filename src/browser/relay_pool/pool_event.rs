//! What the store consumes, after the transport is out of the picture.
//!
//! # Why this exists
//!
//! [`WorkerOut::Note`] carries the relay's whole EVENT frame, while
//! [`Outbound::Note`] carries the note ALREADY PARSED out of that frame. They
//! are not the same JSON, so converting one into the other means either
//! re-encoding a frame the worker already took apart, or emitting bare note
//! JSON that fails to parse as a relay event and vanishes.
//!
//! So the two are not converted into each other. Both convert INTO this type,
//! which is what the store actually wants: a note it can dispatch. The bridge
//! pays the parse it always paid, and the rings pay nothing.

use crate::transport::Outbound;
use nostro2::{NostrNote, NostrRelayEvent};

use super::websocket::ReadyState;
use super::worker::WorkerOut;

/// A worker event, in the form the store dispatches.
pub enum PoolEvent {
    /// A matched note, parsed.
    Note(Box<NostrNote>),
    /// A non-note relay frame, parsed.
    RelayEvent(Box<NostrRelayEvent>),
    /// Per-relay connection state.
    RelayHealth(Vec<(String, ReadyState)>),
}

impl PoolEvent {
    /// Build from a bridge message, parsing what the worker sent as text.
    ///
    /// `None` when the frame does not parse, which is the same outcome the
    /// store had before: an unreadable frame is ignored.
    #[must_use]
    pub fn from_bridge(out: WorkerOut) -> Option<Self> {
        match out {
            WorkerOut::Note(event_json) => match event_json.parse::<NostrRelayEvent>() {
                Ok(NostrRelayEvent::NewNote(.., note)) => Some(Self::Note(Box::new(note))),
                _ => None,
            },
            WorkerOut::RelayEvent(event_json) => event_json
                .parse::<NostrRelayEvent>()
                .ok()
                .map(|event| Self::RelayEvent(Box::new(event))),
            WorkerOut::RelayHealth(updates) => Some(Self::RelayHealth(updates)),
        }
    }

    /// Build from a ring frame. The note is already parsed, so nothing is
    /// decoded here.
    #[must_use]
    pub fn from_wire(message: Outbound) -> Option<Self> {
        match message {
            Outbound::Note(note) => Some(Self::Note(Box::new(note))),
            Outbound::RelayEvent(event_json) => event_json
                .parse::<NostrRelayEvent>()
                .ok()
                .map(|event| Self::RelayEvent(Box::new(event))),
            Outbound::RelayHealth(updates) => Some(Self::RelayHealth(
                super::message_bridge::RelayHealth::from_wire(updates),
            )),
        }
    }
}
