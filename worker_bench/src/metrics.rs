//! Shared benchmark instrumentation, used by both the app (main thread) and
//! the worker.
//!
//! Everything here is allocation-light and avoids touching the DOM so the
//! measurement itself doesn't perturb what it measures.
//!
//! Shared by both bins via `#[path]`; the worker only uses a subset (the note
//! generator + clock), so allow dead code rather than split the module.
#![allow(dead_code)]

/// High-resolution monotonic clock in milliseconds, for SAME-context timing
/// only (e.g. the jank meter, which runs entirely on the main thread).
///
/// Uses `performance.now()` — available on both `Window` and
/// `WorkerGlobalScope`, BUT each context has its OWN time origin, so values
/// are NOT comparable across the worker boundary. Use [`wall_ms`] for that.
#[must_use]
pub fn now_ms() -> f64 {
    use wasm_bindgen::JsCast;
    let global: web_sys::js_sys::Object = web_sys::js_sys::global();
    let perf = web_sys::js_sys::Reflect::get(&global, &"performance".into())
        .ok()
        .and_then(|p| p.dyn_into::<web_sys::Performance>().ok());
    perf.map_or(0.0, |p| p.now())
}

/// Wall-clock (UNIX epoch) milliseconds. Unlike [`now_ms`], `Date.now()` shares
/// the same epoch in the worker and on the main thread, so a timestamp stamped
/// in one context can be subtracted from one read in the other. This is what
/// end-to-end latency must use to survive the worker round-trip.
///
/// Lower resolution than `performance.now()` (often 1ms, sometimes coarsened),
/// but correct across threads — which matters far more here.
#[must_use]
pub fn wall_ms() -> f64 {
    web_sys::js_sys::Date::now()
}

/// A synthetic relay message: a NIP-01 `["EVENT", <sub>, {note}]` JSON string,
/// shaped like what a real relay sends so both paths exercise the same parse.
///
/// The emit timestamp (ms, from [`wall_ms`]) is embedded in the note content as
/// `BENCH:<ms>:<filler>` so end-to-end latency survives the JSON round-trip and
/// the worker boundary. `seq` makes each note id unique (so dedup doesn't drop
/// them) and is also embedded for sanity.
///
/// `payload_bytes` pads the `content` field with that many extra ASCII bytes,
/// to test how message SIZE (not just rate) shifts the in-thread-vs-worker
/// balance: bigger payloads mean more JSON to parse AND more bytes to
/// serialize + structured-clone across the worker boundary.
#[must_use]
pub fn synthetic_event(seq: u64, emit_ms: f64, payload_bytes: usize) -> String {
    // 64-hex id/pubkey/sig so nostro2 parses it as a well-formed note.
    let id = format!("{seq:064x}");
    let pubkey = format!("{:064x}", seq.wrapping_mul(2_654_435_761));
    let sig = format!("{seq:0128x}");
    // created_at in seconds (coarse); real latency rides in the content marker.
    let created_at = (emit_ms / 1000.0) as u64;
    let mut content = format!("BENCH:{emit_ms}:{seq} ");
    // Pad with a JSON-safe ASCII char (no quotes/backslashes/control chars) so
    // nostro2 still parses the note. Reserve to avoid repeated reallocation.
    if payload_bytes > 0 {
        content.reserve(payload_bytes);
        content.extend(std::iter::repeat_n('a', payload_bytes));
    }
    format!(
        r#"["EVENT","bench",{{"id":"{id}","pubkey":"{pubkey}","created_at":{created_at},"kind":1,"tags":[],"content":"{content}","sig":"{sig}"}}]"#
    )
}

/// Extract the embedded emit timestamp (ms) from a benchmark note's content.
/// Returns `None` for non-benchmark notes (e.g. real relay traffic).
#[must_use]
pub fn emit_ms_from_content(content: &str) -> Option<f64> {
    let rest = content.strip_prefix("BENCH:")?;
    let ms_str = rest.split(':').next()?;
    ms_str.parse::<f64>().ok()
}

/// Frame-interval histogram for main-thread jank.
///
/// A responsive 60fps thread holds ~16.7ms between frames. Buckets above that
/// mean the UI thread stalled — the headline cost the worker is meant to avoid.
#[derive(Default, Clone, Copy)]
pub struct JankHistogram {
    pub under_17: u32,
    pub b17_50: u32,
    pub b50_100: u32,
    pub over_100: u32,
    pub worst: f64,
    pub total: u32,
}

impl JankHistogram {
    pub fn record(&mut self, delta_ms: f64) {
        self.total += 1;
        if delta_ms > self.worst {
            self.worst = delta_ms;
        }
        if delta_ms < 17.0 {
            self.under_17 += 1;
        } else if delta_ms < 50.0 {
            self.b17_50 += 1;
        } else if delta_ms < 100.0 {
            self.b50_100 += 1;
        } else {
            self.over_100 += 1;
        }
    }

    /// Fraction of frames that were janky (≥17ms), as a percentage.
    #[must_use]
    pub fn jank_pct(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let janky = self.b17_50 + self.b50_100 + self.over_100;
        f64::from(janky) / f64::from(self.total) * 100.0
    }
}

/// Streaming latency tracker. Keeps a bounded ring of samples and computes
/// percentiles on demand. Bounded so it can't itself become a memory leak.
pub struct Latency {
    samples: Vec<f64>,
    cap: usize,
    next: usize,
    filled: bool,
}

impl Latency {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            samples: Vec::with_capacity(cap),
            cap,
            next: 0,
            filled: false,
        }
    }

    pub fn record(&mut self, ms: f64) {
        if self.samples.len() < self.cap {
            self.samples.push(ms);
        } else {
            self.samples[self.next] = ms;
            self.filled = true;
        }
        self.next = (self.next + 1) % self.cap;
    }

    /// Returns (p50, p95, p99). Empty tracker yields zeros.
    #[must_use]
    pub fn percentiles(&self) -> (f64, f64, f64) {
        if self.samples.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pick = |q: f64| {
            let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
            sorted[idx]
        };
        (pick(0.50), pick(0.95), pick(0.99))
    }
}

/// Per-second throughput counter. Call [`tick`](Self::tick) once per produced
/// item and [`sample`](Self::sample) once per wall-clock second.
#[derive(Default)]
pub struct Throughput {
    pub produced: u64,
    pub rendered: u64,
    pub last_produced: u64,
    pub last_rendered: u64,
    pub per_sec_in: u64,
    pub per_sec_out: u64,
}

impl Throughput {
    pub fn produce(&mut self, n: u64) {
        self.produced += n;
    }
    pub fn render(&mut self, n: u64) {
        self.rendered += n;
    }
    /// Compute the last-second rates; call once per second.
    pub fn sample(&mut self) {
        self.per_sec_in = self.produced - self.last_produced;
        self.per_sec_out = self.rendered - self.last_rendered;
        self.last_produced = self.produced;
        self.last_rendered = self.rendered;
    }
    /// Notes produced but not yet rendered (the backlog / "dropped behind").
    #[must_use]
    pub const fn backlog(&self) -> u64 {
        self.produced.saturating_sub(self.rendered)
    }
}
