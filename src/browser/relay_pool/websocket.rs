//! Relay connection state shared across the worker boundary.
//!
//! The actual `WebSockets` now live INSIDE the worker reactor (see
//! [`super::worker`]); the main thread never holds a `web_sys::WebSocket`. What
//! remains here is [`ReadyState`] — the connection-health enum the worker
//! reports back over the bridge (`WorkerOut::RelayHealth`) and that
//! `relay_health()` hands to consumers. It is `Serialize`/`Deserialize` so it
//! can travel through the JSON codec.

/// A relay socket's connection state, mirroring the WebSocket `readyState`
/// values. Reported by the worker over the bridge.
#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ReadyState {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}

impl ReadyState {
    /// Convert a raw `web_sys` `readyState` (`0..=3`) into a [`ReadyState`].
    ///
    /// Used inside the worker to snapshot each socket's health.
    pub(crate) const fn from_web_sys(state: u16) -> Self {
        match state {
            0 => Self::CONNECTING,
            1 => Self::OPEN,
            2 => Self::CLOSING,
            _ => Self::CLOSED,
        }
    }
}
