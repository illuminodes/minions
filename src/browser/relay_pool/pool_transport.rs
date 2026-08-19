//! The relay pool's transport: how EVERY message moves between the
//! application and the relay worker, in both directions.
//!
//! # One choice, made once
//!
//! Two mechanisms exist:
//!
//! - **Shared rings** — a [`RingPair`] of `SharedArrayBuffer` byte rings
//!   carrying [`wire_envelope`] frames. Needs the `sab-transport` feature AND a
//!   cross-origin isolated page.
//! - **Reactor bridge** — `yew-agent`'s JSON `postMessage`. Always available.
//!
//! The choice happens once, at boot, and then everything rides it: commands,
//! notes, relay events, and health. There is no per-message routing and no
//! second channel running alongside.
//!
//! That is a correctness property, not a preference. A single ordered stream
//! preserves order BETWEEN message kinds, which two parallel channels cannot:
//! a `RelayHealth` saying a socket closed must not overtake the notes that
//! socket already delivered, and a `Close` must not overtake the `Subscribe` it
//! closes.
//!
//! # One worker
//!
//! Both sides call [`PoolTransport`] and neither knows which mechanism is live,
//! so the worker has ONE code path rather than a build per transport. The
//! `#[cfg]` stops at this module.

use wasm_bindgen::prelude::*;
use crate::transport::{Envelope, Inbound, Outbound};

/// How many frames one `receive` call takes at most.
///
/// Bounds the work per turn so a flood cannot monopolise the thread; whatever
/// is left waits for the next call. On the UI thread that call is an animation
/// frame, which is what keeps a burst from becoming a long frame.
#[cfg(feature = "sab-transport")]
pub const RECEIVE_QUOTA: usize = 256;

/// The transport for this thread.
///
/// Build it with [`Self::for_app`] on the UI thread or [`Self::for_worker`] in
/// the worker. When the rings are unavailable, every method degrades to the
/// bridge behaviour rather than failing.
pub struct PoolTransport {
    #[cfg(feature = "sab-transport")]
    endpoint: Option<super::endpoint::Endpoint>,
    #[cfg(feature = "sab-transport")]
    pair: Option<super::ring_pair::RingPair>,
}

impl PoolTransport {
    /// Build the transport in the worker, allocating the rings here.
    ///
    /// The worker owns the allocation because the application has no way to
    /// post a `SharedArrayBuffer` to it; see [`super::handshake`]. The handles
    /// then travel the other way, via [`Self::worker_handles`].
    #[must_use]
    pub fn for_worker_owned() -> Self {
        Self::worker_owned()
    }

    /// Adopt the rings the worker offered, on the application side.
    ///
    /// `None` means no handshake arrived, so the bridge stays in use.
    #[must_use]
    pub fn for_app_attached(handles: Option<&JsValue>) -> Self {
        Self::app_attached(handles)
    }

    /// Whether the shared rings are carrying traffic.
    #[must_use]
    pub fn uses_shared_rings(&self) -> bool {
        self.rings_active()
    }

    /// A human-readable statement of the choice, for one startup log line.
    #[must_use]
    pub fn describe(&self) -> String {
        if self.uses_shared_rings() {
            "relay transport: shared rings (SharedArrayBuffer), all traffic".to_string()
        } else {
            format!("relay transport: JSON bridge, all traffic — {}", Self::why_not_rings())
        }
    }

    /// The handles to post to the worker at boot, when the rings are live.
    ///
    /// `None` means the worker must use the bridge.
    #[must_use]
    pub fn worker_handles(&self) -> Option<JsValue> {
        self.rings_handles()
    }

    /// Send one message to the worker. Never drops it, never blocks.
    ///
    /// Returns `false` when the rings are not in use, meaning the caller must
    /// send this message over the bridge instead.
    pub fn send_to_worker(&self, message: &Inbound) -> bool {
        self.rings_send(&Envelope::encode_inbound(message))
    }

    /// Send one message to the application. Never drops it, never blocks.
    ///
    /// Returns `false` when the rings are not in use, meaning the caller must
    /// send this message over the bridge instead.
    pub fn send_to_app(&self, message: &Outbound) -> bool {
        self.rings_send(&Envelope::encode_outbound(message))
    }

    /// Take up to [`RECEIVE_QUOTA`] messages sent to the application.
    ///
    /// Empty when the bridge is in use — those messages arrive through the
    /// reactor stream instead, so the caller needs no branch of its own.
    #[must_use]
    pub fn receive_outbound(&self) -> Vec<Outbound> {
        self.rings_receive(Envelope::decode_outbound)
    }

    /// Take up to [`RECEIVE_QUOTA`] messages sent to the worker.
    #[must_use]
    pub fn receive_inbound(&self) -> Vec<Inbound> {
        self.rings_receive(Envelope::decode_inbound)
    }

    /// Retry frames the peer's ring refused earlier.
    ///
    /// Call this each turn: it is what makes a full ring a delay rather than a
    /// loss.
    pub fn flush(&self) -> usize {
        self.rings_flush()
    }

    /// Messages still held in this thread's heap because the ring was full.
    /// Non-zero means the peer is behind; it never means data was lost.
    #[must_use]
    pub fn backlog(&self) -> usize {
        self.rings_backlog()
    }

    /// The largest backlog ever reached.
    ///
    /// This is the honest measure of how far the reader fell behind, because
    /// the instantaneous backlog is usually zero. It never means data was lost.
    #[allow(dead_code, reason = "reporting it needs a metrics frame; see worker_sink")]
    #[must_use]
    pub fn peak_backlog(&self) -> usize {
        self.rings_peak_backlog()
    }



    /// Announce that this thread has stopped draining, so the peer stops
    /// encoding for a reader that will never return.
    pub fn close(&self) {
        self.rings_close();
    }
}

#[cfg(feature = "sab-transport")]
#[allow(
    clippy::missing_const_for_fn,
    reason = "signatures must match the non-sab twin, which cannot be const"
)]
impl PoolTransport {
    fn worker_owned() -> Self {
        use super::isolation::Isolation;
        use super::ring_pair::{Role, RingPair};

        if !Isolation::is_available() {
            return Self {
                endpoint: None,
                pair: None,
            };
        }
        let pair = RingPair::create();
        let endpoint = pair.endpoint(Role::Worker);
        endpoint.open();
        Self {
            endpoint: Some(endpoint),
            pair: Some(pair),
        }
    }

    fn app_attached(handles: Option<&JsValue>) -> Self {
        use super::ring_pair::{Role, RingPair};

        let Some(pair) = handles.and_then(|h| RingPair::attach(h).ok()) else {
            return Self {
                endpoint: None,
                pair: None,
            };
        };
        let endpoint = pair.endpoint(Role::App);
        endpoint.open();
        Self {
            endpoint: Some(endpoint),
            pair: Some(pair),
        }
    }

    fn rings_active(&self) -> bool {
        self.endpoint.is_some()
    }

    fn rings_handles(&self) -> Option<JsValue> {
        self.pair.as_ref().map(super::ring_pair::RingPair::handles)
    }

    fn rings_send(&self, frame: &[u8]) -> bool {
        let Some(endpoint) = self.endpoint.as_ref() else {
            return false;
        };
        endpoint.send(frame.to_vec());
        true
    }

    fn rings_receive<T>(&self, decode: impl Fn(&[u8]) -> Option<T>) -> Vec<T> {
        let Some(endpoint) = self.endpoint.as_ref() else {
            return Vec::new();
        };
        endpoint
            .recv(RECEIVE_QUOTA)
            .iter()
            .filter_map(|frame| decode(frame))
            .collect()
    }

    fn rings_flush(&self) -> usize {
        self.endpoint.as_ref().map_or(0, super::endpoint::Endpoint::flush)
    }

    fn rings_backlog(&self) -> usize {
        self.endpoint
            .as_ref()
            .map_or(0, super::endpoint::Endpoint::backlog)
    }

    #[allow(dead_code, reason = "reporting it needs a metrics frame; see worker_sink")]
    fn rings_peak_backlog(&self) -> usize {
        self.endpoint
            .as_ref()
            .map_or(0, super::endpoint::Endpoint::peak_backlog)
    }



    fn rings_close(&self) {
        if let Some(endpoint) = self.endpoint.as_ref() {
            endpoint.close();
        }
    }

    fn why_not_rings() -> String {
        super::isolation::Isolation::explain().to_string()
    }
}

/// The no-op twin. Every method mirrors the `sab-transport` signature exactly
/// so the public API above needs no `#[cfg]` of its own, and so the worker
/// keeps ONE code path in both builds.
///
/// `unused_self` and `missing_const_for_fn` fire here by construction: these
/// bodies ignore `self` precisely BECAUSE there is no ring to consult. Do NOT
/// take clippy's advice to make them `const` — the twin above cannot be, and
/// the public wrappers call both.
#[cfg(not(feature = "sab-transport"))]
#[allow(clippy::unused_self, clippy::missing_const_for_fn)]
impl PoolTransport {
    fn worker_owned() -> Self {
        Self {}
    }

    fn app_attached(_handles: Option<&JsValue>) -> Self {
        Self {}
    }

    fn rings_active(&self) -> bool {
        false
    }

    fn rings_handles(&self) -> Option<JsValue> {
        None
    }

    fn rings_send(&self, _frame: &[u8]) -> bool {
        false
    }

    fn rings_receive<T>(&self, _decode: impl Fn(&[u8]) -> Option<T>) -> Vec<T> {
        Vec::new()
    }

    fn rings_flush(&self) -> usize {
        0
    }

    fn rings_backlog(&self) -> usize {
        0
    }

    #[allow(dead_code, reason = "reporting it needs a metrics frame; see worker_sink")]
    fn rings_peak_backlog(&self) -> usize {
        0
    }

    fn rings_close(&self) {}

    fn why_not_rings() -> String {
        "built without the `sab-transport` feature".to_string()
    }
}

impl Drop for PoolTransport {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for PoolTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolTransport")
            .field("shared_rings", &self.uses_shared_rings())
            .field("backlog", &self.backlog())
            .finish()
    }
}
