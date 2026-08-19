//! The worker's outbound side: one place that decides where a message goes.
//!
//! The reactor loop calls [`WorkerSink::ship`] and never learns which transport
//! is live. That is what keeps ONE worker: the loop, the sockets, and the
//! ingestor are identical in every build and on every page, and only this type
//! branches.
//!
//! The branch is a runtime `if`, not a `#[cfg]`, because the same binary must
//! serve an isolated page (rings) and a non-isolated one (bridge).

use futures::SinkExt;
use yew_agent::reactor::ReactorScope;

use super::ingested::Ingested;
use super::message_bridge::RelayHealth;
use super::pool_transport::PoolTransport;
use super::websocket::ReadyState;
use super::worker::{RelayCommand, WorkerOut};

/// Sends worker output over whichever transport the application chose.
pub struct WorkerSink {
    transport: std::rc::Rc<PoolTransport>,
}

impl WorkerSink {
    #[must_use]
    pub const fn new(transport: std::rc::Rc<PoolTransport>) -> Self {
        Self { transport }
    }

    /// Whether the shared rings carry this worker's traffic.
    #[must_use]
    pub fn uses_shared_rings(&self) -> bool {
        self.transport.uses_shared_rings()
    }

    /// Ship one ingested frame. Returns `false` when the peer is gone.
    ///
    /// Each transport takes the form that costs it nothing: the rings take the
    /// parsed note, the bridge takes the relay's original string.
    pub async fn ship(
        &self,
        ingested: Ingested,
        scope: &mut ReactorScope<RelayCommand, WorkerOut>,
    ) -> bool {
        if self.transport.uses_shared_rings() {
            self.transport.send_to_app(&ingested.into_wire());
            self.transport.flush();
            return true;
        }
        scope.send(ingested.into_bridge()).await.is_ok()
    }

    /// Ship a health update. Returns `false` when the peer is gone.
    pub async fn ship_health(
        &self,
        health: Vec<(String, ReadyState)>,
        scope: &mut ReactorScope<RelayCommand, WorkerOut>,
    ) -> bool {
        if self.transport.uses_shared_rings() {
            self.transport.send_to_app(&RelayHealth::to_wire(health));
            self.transport.flush();
            return true;
        }
        scope.send(WorkerOut::RelayHealth(health)).await.is_ok()
    }

    /// Take the commands waiting in the ring, converted to the worker's own
    /// vocabulary. Empty when the bridge is in use.
    #[must_use]
    pub fn poll_commands(&self) -> Vec<RelayCommand> {
        self.transport
            .receive_inbound()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Retry frames a full ring refused earlier.
    pub fn flush(&self) -> usize {
        self.transport.flush()
    }

    /// Frames held in the worker's heap because the ring was full.
    ///
    /// Not yet reported to the application. Sending it needs a metrics frame in
    /// [`wire_envelope`], which is a wire change with its own tests, so the
    /// benchmark's queue-depth readout stays at zero until then.
    #[allow(
        dead_code,
        reason = "reporting it needs a metrics frame in wire_envelope"
    )]
    #[must_use]
    pub fn backlog(&self) -> usize {
        self.transport.backlog()
    }

    /// The worst backlog this worker ever reached.
    ///
    /// The momentary backlog is almost always zero even when the reader stalled
    /// badly, so this is the honest number to report.
    #[allow(
        dead_code,
        reason = "reporting it needs a metrics frame in wire_envelope"
    )]
    #[must_use]
    pub fn peak_backlog(&self) -> usize {
        self.transport.peak_backlog()
    }
}
