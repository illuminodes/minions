//! The application's side of the transport, and the driver loop that runs it.
//!
//! The worker offers the rings after it boots (see [`super::handshake`]), so
//! this side starts on the bridge and adopts the rings when they arrive. Both
//! directions move together: once the rings are live they carry commands AND
//! output, and the bridge carries neither.
//!
//! Commands sent before the handshake are not lost. They go over the bridge,
//! which is connected from the first frame.

use wasm_bindgen::JsValue;

use super::handshake::RingHandshake;
use super::pool_event::PoolEvent;
use super::pool_transport::PoolTransport;
use super::worker::RelayCommand;

/// The application's transport, which upgrades once.
pub struct AppLink {
    transport: PoolTransport,
}

impl AppLink {
    /// Start on the bridge, with no rings yet.
    #[must_use]
    pub fn pending() -> Self {
        Self {
            transport: PoolTransport::for_app_attached(None),
        }
    }

    /// Adopt the rings if the worker has offered them since the last check.
    ///
    /// Returns `true` on the turn the upgrade happens, so the caller can log it
    /// once.
    pub fn adopt_offered_rings(&mut self) -> bool {
        if self.uses_shared_rings() {
            return false;
        }
        let Some(handles) = RingHandshake::handles() else {
            return false;
        };
        let upgraded = Self::attach(&handles);
        if !upgraded.uses_shared_rings() {
            return false;
        }
        self.transport = upgraded.transport;
        true
    }

    fn attach(handles: &JsValue) -> Self {
        Self {
            transport: PoolTransport::for_app_attached(Some(handles)),
        }
    }

    /// Whether the rings carry this application's traffic.
    #[must_use]
    pub fn uses_shared_rings(&self) -> bool {
        self.transport.uses_shared_rings()
    }

    /// A one-line statement of the transport in use.
    #[must_use]
    pub fn describe(&self) -> String {
        self.transport.describe()
    }

    /// Send a command, and report whether the rings took it.
    ///
    /// `false` means the caller must use the bridge. The command is never sent
    /// twice, and never dropped.
    pub fn send(&self, command: &RelayCommand) -> bool {
        if !self.uses_shared_rings() {
            return false;
        }
        let sent = self.transport.send_to_worker(&command.clone().into());
        self.transport.flush();
        sent
    }

    /// Take the worker output waiting in the ring.
    #[must_use]
    pub fn drain(&self) -> Vec<PoolEvent> {
        self.transport
            .receive_outbound()
            .into_iter()
            .filter_map(PoolEvent::from_wire)
            .collect()
    }

    /// Frames held back because the ring was full.
    ///
    /// On this side the ring carries commands, which are rare, so this is
    /// expected to stay at zero; a non-zero value means the worker stopped
    /// reading.
    #[allow(dead_code, reason = "command backlog is not surfaced to consumers yet")]
    #[must_use]
    pub fn backlog(&self) -> usize {
        self.transport.backlog()
    }
}
