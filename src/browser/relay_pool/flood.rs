//! Synthetic load generator for the relay reactor, used by `worker_bench`.
//!
//! The generator feeds frames through the SAME [`NoteIngestor`] the sockets
//! use, so a benchmark measures the shipped ingestion path rather than a copy
//! of it. It lives behind the `bench-harness` feature and compiles to nothing
//! in a normal build.

use super::ingest::NoteIngestor;

/// One synthetic NIP-01 `["EVENT", …]` frame, shaped like real relay traffic so
/// the parse cost is representative.
///
/// The emit timestamp rides inside `content` as `BENCH:<ms>:<seq>` so
/// end-to-end latency survives the worker round-trip, and `seq` keeps every id
/// unique so dedup does not swallow the load.
pub struct SyntheticFrame {
    seq: u64,
    emit_ms: f64,
    payload_bytes: usize,
}

impl SyntheticFrame {
    #[must_use]
    pub const fn new(seq: u64, emit_ms: f64, payload_bytes: usize) -> Self {
        Self {
            seq,
            emit_ms,
            payload_bytes,
        }
    }

    #[must_use]
    pub fn render(&self) -> String {
        let id = format!("{:064x}", self.seq);
        let pubkey = format!("{:064x}", self.seq.wrapping_mul(2_654_435_761));
        let sig = format!("{:0128x}", self.seq);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let created_at = (self.emit_ms / 1000.0) as u64;
        let mut content = format!("BENCH:{}:{} ", self.emit_ms, self.seq);
        if self.payload_bytes > 0 {
            content.reserve(self.payload_bytes);
            content.extend(std::iter::repeat_n('a', self.payload_bytes));
        }
        format!(
            r#"["EVENT","bench",{{"id":"{id}","pubkey":"{pubkey}","created_at":{created_at},"kind":1,"tags":[],"content":"{content}","sig":"{sig}"}}]"#
        )
    }
}

/// Parameters for one flood run.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FloodSpec {
    pub rate: u32,
    pub secs: u32,
    pub start_seq: u64,
    pub payload_bytes: usize,
}

/// Drives a [`FloodSpec`] through a [`NoteIngestor`], emitting whatever the
/// ingestor decides to ship.
///
/// Frames are emitted in ~16ms ticks so they spread over wall-clock time like a
/// real high-rate feed, instead of arriving as one blocking burst.
pub struct FloodRunner {
    ingestor: NoteIngestor,
}

impl FloodRunner {
    #[must_use]
    pub const fn new(ingestor: NoteIngestor) -> Self {
        Self { ingestor }
    }

    /// `emit` receives exactly what a real relay frame produces, so the flood
    /// exercises the live transport instead of a parallel one.
    pub fn run<F>(&self, spec: FloodSpec, mut emit: F)
    where
        F: FnMut(super::ingested::Ingested) -> bool + 'static,
    {
        const TICK_MS: u32 = 16;
        let ingestor = self.ingestor.clone();
        yew::platform::spawn_local(async move {
            let ticks = spec.secs * (1000 / TICK_MS);
            let per_tick = (spec.rate * TICK_MS / 1000).max(1);
            let mut seq = spec.start_seq;

            for _ in 0..ticks {
                let emit_ms = web_sys::js_sys::Date::now();
                for _ in 0..per_tick {
                    let raw = SyntheticFrame::new(seq, emit_ms, spec.payload_bytes).render();
                    seq += 1;
                    if let Some(out) = ingestor.ingest(raw) {
                        if !emit(out) {
                            return;
                        }
                    }
                }
                yew::platform::time::sleep(std::time::Duration::from_millis(u64::from(TICK_MS)))
                    .await;
            }
        });
    }
}
