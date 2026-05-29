//! Benchmark application (UI thread / `data-type="main"`).
//!
//! Compares two relay-ingestion architectures under a controlled synthetic
//! load, with quantitative instrumentation — because at live-relay rates both
//! are trivially fast and indistinguishable by eye.
//!
//! - **In-thread**: parse + dedup + filter run on the UI thread (what the
//!   `nostr-minions` pool does in its socket `onmessage`). The flood feeder
//!   runs that same work on the main thread.
//! - **Worker**: the `RelayReactor` does that work off-thread and streams
//!   matched notes back across the bridge (JSON encode + postMessage).
//!
//! Instrumentation (see `metrics.rs`), all sampled identically for both paths:
//! - main-thread jank histogram (rAF frame intervals)
//! - throughput (produced vs rendered per second, + backlog)
//! - per-note end-to-end latency (p50/p95/p99), via a timestamp embedded in
//!   each synthetic note's content so it survives the worker round-trip
//! - a once-per-second `[BENCH]` `console.table` dump

#[path = "relay_worker.rs"]
mod relay_worker;
// Reuse the metrics module already loaded by relay_worker; loading metrics.rs
// a second time via #[path] would define it twice in this bin.
use relay_worker::metrics;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use metrics::{JankHistogram, Latency, Throughput};
use nostro2::{NostrNote, NostrRelayEvent, NostrSubscription};
use relay_worker::{JsonCodec, RelayCommand, RelayReactor};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_agent::reactor::{use_reactor_bridge, ReactorProvider};

const LIMIT: u32 = 50;
const FLOOD_SECS: u32 = 5;
/// Max notes the worker panel pulls off the bridge per animation frame. Caps
/// per-frame main-thread work so a fast worker stream can't block the UI in one
/// synchronous burst — the rest waits for the next frame.
const DRAIN_PER_FRAME: usize = 200;

fn kind1_filter() -> NostrSubscription {
    NostrSubscription {
        kinds: Some(vec![1]),
        limit: Some(LIMIT),
        ..Default::default()
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}

/// Self-rescheduling `requestAnimationFrame` callback handle.
type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Shared, mutable benchmark state. One instance lives at the app root and is
/// updated by whichever path is active and read by the metrics UI.
#[derive(Default)]
struct Metrics {
    jank: JankHistogram,
    throughput: Throughput,
    latency_samples: Option<Latency>,
    label: &'static str,
    /// Worker-side bridge queue depth (notes matched but not yet shipped
    /// across the bridge). Reported by the worker; always 0 for in-thread.
    worker_queue: u64,
}

// The metrics cell is a singleton shared by reference across the app, so all
// props referencing it are equal. This satisfies the `Properties` PartialEq
// bound without diffing the interior (which mutates constantly); components
// re-render from their own state/timers, not from prop changes here.
impl PartialEq for Metrics {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Metrics {
    fn latency(&mut self) -> &mut Latency {
        self.latency_samples
            .get_or_insert_with(|| Latency::new(4096))
    }
    /// Reset everything except keep the latency buffer allocated.
    fn reset(&mut self, label: &'static str) {
        self.jank = JankHistogram::default();
        self.throughput = Throughput::default();
        self.latency_samples = Some(Latency::new(4096));
        self.label = label;
        self.worker_queue = 0;
    }
}

type SharedMetrics = Rc<RefCell<Metrics>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Path {
    None,
    InThread,
    Worker,
}

#[function_component(App)]
fn app() -> Html {
    let path = use_state(|| Path::None);
    let rate = use_state(|| 2000u32);
    // Extra bytes padded into each note's content — the message-SIZE axis.
    let payload = use_state(|| 0u32);
    let metrics: SharedMetrics = use_mut_ref(Metrics::default);

    // Continuous jank sampling + once-per-second throughput sample & console
    // dump. Runs for the lifetime of the app, regardless of active path.
    use_bench_sampler(metrics.clone());

    let switch = {
        let path = path.clone();
        let metrics = metrics.clone();
        move |target: Path, label: &'static str| {
            let path = path.clone();
            let metrics = metrics.clone();
            Callback::from(move |_| {
                metrics.borrow_mut().reset(label);
                path.set(target);
            })
        }
    };

    let btn = |label: &'static str, target: Path, on: Callback<MouseEvent>, color: &str| {
        let active = *path == target;
        let bg = if active { color } else { "#e5e7eb" };
        let fg = if active { "white" } else { "#374151" };
        html! {
            <button onclick={on} style={format!(
                "padding:.5rem 1rem; margin:0 .25rem; border:none; border-radius:6px; \
                 cursor:pointer; font-weight:600; background:{bg}; color:{fg};"
            )}>{label}</button>
        }
    };

    let on_rate = {
        let rate = rate.clone();
        Callback::from(move |e: InputEvent| {
            let v = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            if let Ok(n) = v.parse::<u32>() {
                rate.set(n);
            }
        })
    };

    let on_payload = {
        let payload = payload.clone();
        Callback::from(move |e: InputEvent| {
            let v = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            if let Ok(n) = v.parse::<u32>() {
                payload.set(n);
            }
        })
    };

    html! {
        <div style="font-family:system-ui; padding:1rem; background:#f3f4f6; min-height:100vh;">
            <h1 style="text-align:center; margin:.2rem;">{"Relay Pool Benchmark"}</h1>
            <p style="text-align:center; font-size:.8rem; color:#6b7280; margin:.2rem;">
                {"One path at a time. Flood injects synthetic notes through the same parse+dedup+filter path each architecture really uses."}
            </p>

            <div style="text-align:center; margin:.5rem;">
                { btn("In-Thread", Path::InThread, switch(Path::InThread, "in-thread"), "#2563eb") }
                { btn("Web Worker", Path::Worker, switch(Path::Worker, "worker"), "#16a34a") }
                { btn("Stop", Path::None, switch(Path::None, "idle"), "#6b7280") }
                <span style="margin-left:1rem; font-size:.85rem;">
                    {"rate (notes/sec): "}
                    <input type="number" min="100" max="20000" step="100"
                        value={rate.to_string()} oninput={on_rate}
                        style="width:6rem; padding:.2rem;" />
                </span>
                <span style="margin-left:1rem; font-size:.85rem;">
                    {"payload (bytes/note): "}
                    <input type="number" min="0" max="1000000" step="500"
                        value={payload.to_string()} oninput={on_payload}
                        style="width:7rem; padding:.2rem;" />
                </span>
            </div>

            <SmoothnessMeter />
            <MetricsPanel metrics={metrics.clone()} />

            <div style="max-width:680px; margin:0 auto;">
                { match *path {
                    Path::None => html!{
                        <p style="text-align:center; color:#9ca3af; font-style:italic; padding:2rem;">
                            {"Pick a path, then click Flood inside it."}
                        </p>
                    },
                    Path::InThread => html!{
                        <InThreadPanel key="in-thread" metrics={metrics.clone()} rate={*rate} payload={*payload} />
                    },
                    Path::Worker => html!{
                        <ReactorProvider<RelayReactor, JsonCodec> key="worker" path="/worker.js">
                            <WorkerPanel metrics={metrics.clone()} rate={*rate} payload={*payload} />
                        </ReactorProvider<RelayReactor, JsonCodec>>
                    },
                }}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PathProps {
    metrics: SharedMetrics,
    rate: u32,
    payload: u32,
}

/// In-thread path: the flood feeder generates raw relay-message JSON and runs
/// parse + dedup + filter ON THE MAIN THREAD (mirroring the pool's onmessage),
/// then renders. This is exactly the work the worker offloads.
#[function_component(InThreadPanel)]
fn in_thread_panel(props: &PathProps) -> Html {
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);
    let force = use_force_update();
    let seq = use_mut_ref(|| 0u64);
    let dedup = use_mut_ref(|| nostr_minions::BoundedDedup::new(10_000));

    let flood = {
        let metrics = props.metrics.clone();
        let rate = props.rate;
        let payload = props.payload as usize;
        let notes = notes.clone();
        let force = force.clone();
        let seq = seq.clone();
        let dedup = dedup.clone();
        Callback::from(move |_| {
            let metrics = metrics.clone();
            let notes = notes.clone();
            let force = force.clone();
            let seq = seq.clone();
            let dedup = dedup.clone();
            let filter = kind1_filter();
            yew::platform::spawn_local(async move {
                // Coarse burst cadence (NOT per-frame): a real relay socket
                // hands you a whole batch in one onmessage call, and ALL of
                // that parse+dedup+filter runs synchronously on the main thread
                // before the event loop can paint. 100ms bursts make that block
                // visible as dropped frames — the cost the worker avoids.
                const BURST_MS: u32 = 100;
                let bursts = FLOOD_SECS * (1000 / BURST_MS);
                let per_burst = (rate * BURST_MS / 1000).max(1);
                for _ in 0..bursts {
                    let emit = metrics::wall_ms();
                    let mut produced = 0u64;
                    let mut rendered = 0u64;
                    // Tight, non-yielding loop — blocks the main thread.
                    for _ in 0..per_burst {
                        let s = *seq.borrow();
                        *seq.borrow_mut() += 1;
                        produced += 1;
                        let raw = metrics::synthetic_event(s, emit, payload);
                        // SAME work the worker does — just on the UI thread.
                        let Ok(NostrRelayEvent::NewNote(.., note)) = raw.parse::<NostrRelayEvent>()
                        else {
                            continue;
                        };
                        if let Some(ref id) = note.id {
                            if !dedup.borrow_mut().insert(id.clone()) {
                                continue;
                            }
                        }
                        if !nostr_minions::note_matches_filter(&note, &filter) {
                            continue;
                        }
                        // Latency: arrival is now; emit is embedded in content.
                        if let Some(em) = metrics::emit_ms_from_content(&note.content) {
                            metrics
                                .borrow_mut()
                                .latency()
                                .record(metrics::wall_ms() - em);
                        }
                        let mut buf = notes.borrow_mut();
                        buf.push_front(note);
                        buf.truncate(LIMIT as usize);
                        rendered += 1;
                    }
                    {
                        let mut m = metrics.borrow_mut();
                        m.throughput.produce(produced);
                        m.throughput.render(rendered);
                    }
                    force.force_update();
                    yew::platform::time::sleep(Duration::from_millis(u64::from(BURST_MS))).await;
                }
            });
        })
    };

    *renders.borrow_mut() += 1;
    let snapshot: Vec<NostrNote> = notes.borrow().iter().cloned().collect();
    html! {
        <Panel title="In-Thread (main-thread parse)" color="#2563eb"
            render_count={*renders.borrow()} notes={snapshot} on_flood={flood} />
    }
}

/// Worker path: the flood runs inside the worker; matched notes stream back.
///
/// Uses `use_reactor_bridge` (callback per output) — NOT
/// `use_reactor_subscription`, which accumulates every output into a Vec and
/// forces a re-render on EVERY message (thousands/sec under load, each cloning
/// the growing history). The callback approach mirrors the in-thread path: push
/// into a local buffer, and re-render once per frame via our own rAF loop.
#[function_component(WorkerPanel)]
fn worker_panel(props: &PathProps) -> Html {
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);
    let seq = use_mut_ref(|| 1u64);
    // Notes land here from the bridge callback; the rAF loop moves them into
    // `notes` and renders once per frame. Decouples arrival rate from renders.
    let pending = use_mut_ref(VecDeque::<NostrNote>::new);

    let bridge = {
        let pending = pending.clone();
        let metrics = props.metrics.clone();
        use_reactor_bridge::<RelayReactor, _>(move |ev| {
            if let yew_agent::reactor::ReactorEvent::Output(out) = ev {
                match out {
                    relay_worker::WorkerOut::Note(note) => {
                        // Per-note work only — no render here.
                        if let Some(em) = metrics::emit_ms_from_content(&note.content) {
                            metrics
                                .borrow_mut()
                                .latency()
                                .record(metrics::wall_ms() - em);
                        }
                        metrics.borrow_mut().throughput.produce(1);
                        pending.borrow_mut().push_back(note);
                    }
                    // The hidden bridge backlog the app's own metric can't see.
                    relay_worker::WorkerOut::QueueDepth(depth) => {
                        metrics.borrow_mut().worker_queue = depth;
                    }
                }
            }
        })
    };

    // Establish the filter once so the worker matches our synthetic notes.
    use_effect_with((), {
        let bridge = bridge.clone();
        move |()| {
            bridge.send(RelayCommand::Subscribe(kind1_filter()));
            || ()
        }
    });

    let flood = {
        let bridge = bridge.clone();
        let rate = props.rate;
        let payload = props.payload as usize;
        let seq = seq.clone();
        Callback::from(move |_| {
            let start = *seq.borrow();
            *seq.borrow_mut() += u64::from(rate) * u64::from(FLOOD_SECS) + 1;
            bridge.send(RelayCommand::Flood {
                rate,
                secs: FLOOD_SECS,
                start_seq: start,
                payload_bytes: payload,
            });
        })
    };

    // rAF loop: move up to DRAIN_PER_FRAME pending notes into the render buffer
    // and re-render ONCE per frame, only when something arrived.
    {
        let notes = notes.clone();
        let pending = pending.clone();
        let metrics = props.metrics.clone();
        let force = use_force_update();
        use_effect_with((), move |()| {
            let cb: RafClosure = Rc::new(RefCell::new(None));
            let cb2 = cb.clone();
            *cb.borrow_mut() = Some(Closure::wrap(Box::new(move |_ts: f64| {
                let drained = {
                    let mut pend = pending.borrow_mut();
                    if pend.is_empty() {
                        0usize
                    } else {
                        let take = pend.len().min(DRAIN_PER_FRAME);
                        let mut buf = notes.borrow_mut();
                        for _ in 0..take {
                            if let Some(note) = pend.pop_front() {
                                buf.push_front(note);
                            }
                        }
                        buf.truncate(LIMIT as usize);
                        take
                    }
                };
                if drained > 0 {
                    metrics.borrow_mut().throughput.render(drained as u64);
                    force.force_update();
                }
                if let (Some(win), Some(c)) = (web_sys::window(), cb2.borrow().as_ref()) {
                    let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut(f64)>));
            if let (Some(win), Some(c)) = (web_sys::window(), cb.borrow().as_ref()) {
                let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
            }
            move || drop(cb)
        });
    }

    *renders.borrow_mut() += 1;
    let snapshot: Vec<NostrNote> = notes.borrow().iter().cloned().collect();
    html! {
        <Panel title="Web Worker (off-thread parse)" color="#16a34a"
            render_count={*renders.borrow()} notes={snapshot} on_flood={flood} />
    }
}

#[derive(Properties, PartialEq)]
struct PanelProps {
    title: &'static str,
    color: &'static str,
    render_count: usize,
    notes: Vec<NostrNote>,
    on_flood: Callback<MouseEvent>,
}

#[function_component(Panel)]
fn panel(props: &PanelProps) -> Html {
    html! {
        <div style="background:white; border-radius:8px; padding:1rem; box-shadow:0 1px 3px rgba(0,0,0,.1); display:flex; flex-direction:column; height:55vh;">
            <div style="display:flex; justify-content:space-between; align-items:center;">
                <h2 style={format!("color:{}; margin:0; font-size:1.1rem;", props.color)}>{props.title}</h2>
                <div style="display:flex; gap:.75rem; align-items:center;">
                    <span style="font-size:.75rem; color:#6b7280;">
                        {format!("renders: {} · notes: {}", props.render_count, props.notes.len())}
                    </span>
                    <button onclick={props.on_flood.clone()} style={format!(
                        "padding:.4rem .8rem; border:none; border-radius:6px; cursor:pointer; \
                         font-weight:700; background:{}; color:white;", props.color
                    )}>{format!("Flood {FLOOD_SECS}s")}</button>
                </div>
            </div>
            <div style="overflow-y:auto; margin-top:.5rem; flex:1; min-height:0;">
                { if props.notes.is_empty() {
                    html!{ <p style="color:#9ca3af; font-style:italic;">{"no notes yet — click Flood"}</p> }
                } else {
                    html!{ <> { for props.notes.iter().take(LIMIT as usize).map(render_note) } </> }
                }}
            </div>
        </div>
    }
}

fn render_note(note: &NostrNote) -> Html {
    let from = note.pubkey.get(..12).unwrap_or(&note.pubkey);
    html! {
        <div style="padding:.35rem; background:#f9fafb; border:1px solid #e5e7eb; border-radius:6px; margin-bottom:.3rem;">
            <div style="font-size:.65rem; color:#6b7280; font-family:monospace;">{format!("{from}…")}</div>
            <div style="font-size:.8rem; color:#111827; overflow:hidden;">
                { note.content.chars().take(80).collect::<String>() }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MetricsProps {
    metrics: SharedMetrics,
}

/// Live metrics readout. Re-renders on a timer driven by the sampler.
#[function_component(MetricsPanel)]
fn metrics_panel(props: &MetricsProps) -> Html {
    let tick = use_state(|| 0u32);
    // Re-render 4×/sec so numbers update smoothly without thrashing.
    use_effect_with((), {
        let tick = tick.clone();
        move |()| {
            let handle = gloo_like_interval(250, move || tick.set(*tick + 1));
            move || drop(handle)
        }
    });

    let m = props.metrics.borrow();
    let (p50, p95, p99) = m
        .latency_samples
        .as_ref()
        .map_or((0.0, 0.0, 0.0), Latency::percentiles);
    let j = &m.jank;
    let t = &m.throughput;

    let cell = |label: &str, value: String, color: &str| {
        html! {
            <div style="text-align:center; padding:.3rem .6rem;">
                <div style={format!("font-size:1.1rem; font-weight:700; color:{color};")}>{value}</div>
                <div style="font-size:.65rem; color:#6b7280; text-transform:uppercase;">{label}</div>
            </div>
        }
    };

    html! {
        <div style="max-width:680px; margin:0 auto .8rem; background:white; border-radius:8px; padding:.6rem; box-shadow:0 1px 3px rgba(0,0,0,.1);">
            <div style="display:flex; justify-content:space-around; flex-wrap:wrap;">
                { cell("path", m.label.to_string(), "#111827") }
                { cell("in /s", t.per_sec_in.to_string(), "#2563eb") }
                { cell("out /s", t.per_sec_out.to_string(), "#16a34a") }
                { cell("backlog", t.backlog().to_string(),
                    if t.backlog() > 1000 { "#dc2626" } else { "#111827" }) }
                { cell("wkr queue", m.worker_queue.to_string(),
                    if m.worker_queue > 1000 { "#dc2626" } else { "#111827" }) }
                { cell("lat p50", format!("{p50:.1}ms"), "#111827") }
                { cell("lat p95", format!("{p95:.1}ms"), "#d97706") }
                { cell("lat p99", format!("{p99:.1}ms"), "#dc2626") }
            </div>
            <div style="display:flex; justify-content:space-around; flex-wrap:wrap; border-top:1px solid #f3f4f6; margin-top:.3rem; padding-top:.3rem;">
                { cell("frames", j.total.to_string(), "#6b7280") }
                { cell("<17ms", j.under_17.to_string(), "#16a34a") }
                { cell("17-50", j.b17_50.to_string(), "#d97706") }
                { cell("50-100", j.b50_100.to_string(), "#ea580c") }
                { cell("100ms+", j.over_100.to_string(), "#dc2626") }
                { cell("worst", format!("{:.0}ms", j.worst), "#dc2626") }
                { cell("jank %", format!("{:.1}", j.jank_pct()), "#dc2626") }
            </div>
        </div>
    }
}

/// A main-thread-driven smoothness indicator. A JS `requestAnimationFrame`
/// loop advances a rotating bar and computes a rolling FPS. Because it runs ON
/// the main thread, it visibly FREEZES when the main thread is blocked (the
/// in-thread flood's parse bursts) and stays smooth when work is off-thread
/// (the worker). This is the human-visible version of the jank histogram.
#[function_component(SmoothnessMeter)]
fn smoothness_meter() -> Html {
    let angle = use_state(|| 0f64);
    let fps = use_state(|| 0f64);

    use_effect_with((), {
        let angle = angle.clone();
        let fps = fps.clone();
        move |()| {
            let last = Rc::new(RefCell::new(0f64));
            let cb: RafClosure = Rc::new(RefCell::new(None));
            let cb2 = cb.clone();
            *cb.borrow_mut() = Some(Closure::wrap(Box::new(move |ts: f64| {
                let prev = *last.borrow();
                if prev > 0.0 {
                    let dt = ts - prev;
                    if dt > 0.0 {
                        // Smooth the FPS a little so the number is readable.
                        let inst = 1000.0 / dt;
                        fps.set((*fps).mul_add(0.8, inst * 0.2));
                    }
                }
                *last.borrow_mut() = ts;
                angle.set((*angle + 6.0) % 360.0);
                if let (Some(win), Some(c)) = (web_sys::window(), cb2.borrow().as_ref()) {
                    let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut(f64)>));
            if let (Some(win), Some(c)) = (web_sys::window(), cb.borrow().as_ref()) {
                let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
            }
            move || drop(cb)
        }
    });

    let fps_now = *fps;
    let fps_color = if fps_now >= 50.0 {
        "#16a34a"
    } else if fps_now >= 30.0 {
        "#d97706"
    } else {
        "#dc2626"
    };
    html! {
        <div style="display:flex; align-items:center; justify-content:center; gap:.75rem; margin:.4rem;">
            <div style={format!(
                "width:28px; height:28px; border:4px solid #d1d5db; border-top-color:#2563eb; \
                 border-radius:50%; transform:rotate({}deg);", *angle
            )}></div>
            <span style="font-size:.85rem; color:#6b7280;">{"main-thread animation — "}</span>
            <span style={format!("font-size:.95rem; font-weight:700; color:{fps_color};")}>
                {format!("{fps_now:.0} fps")}
            </span>
            <span style="font-size:.75rem; color:#9ca3af;">{"(stutters when the UI thread blocks)"}</span>
        </div>
    }
}

/// Drives jank sampling (every animation frame) and a 1Hz throughput sample +
/// `[BENCH]` console dump. Independent of the active path.
#[hook]
fn use_bench_sampler(metrics: SharedMetrics) {
    // rAF loop → jank histogram.
    {
        let metrics = metrics.clone();
        use_effect_with((), move |()| {
            let last = Rc::new(RefCell::new(0f64));
            let cb: RafClosure = Rc::new(RefCell::new(None));
            let cb2 = cb.clone();
            *cb.borrow_mut() = Some(Closure::wrap(Box::new(move |ts: f64| {
                let prev = *last.borrow();
                if prev > 0.0 {
                    metrics.borrow_mut().jank.record(ts - prev);
                }
                *last.borrow_mut() = ts;
                if let (Some(win), Some(c)) = (web_sys::window(), cb2.borrow().as_ref()) {
                    let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut(f64)>));
            if let (Some(win), Some(c)) = (web_sys::window(), cb.borrow().as_ref()) {
                let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
            }
            move || drop(cb)
        });
    }

    // 1Hz throughput sample + structured console dump.
    use_effect_with((), move |()| {
        let handle = gloo_like_interval(1000, move || {
            let mut m = metrics.borrow_mut();
            m.throughput.sample();
            let (p50, p95, p99) = m
                .latency_samples
                .as_ref()
                .map_or((0.0, 0.0, 0.0), Latency::percentiles);
            // Structured dump — copyable from the console.
            let dump = web_sys::js_sys::Object::new();
            let set = |k: &str, v: JsValue| {
                let _ = web_sys::js_sys::Reflect::set(&dump, &k.into(), &v);
            };
            set("path", m.label.into());
            set(
                "in_per_s",
                f64::from(u32::try_from(m.throughput.per_sec_in).unwrap_or(u32::MAX)).into(),
            );
            set(
                "out_per_s",
                f64::from(u32::try_from(m.throughput.per_sec_out).unwrap_or(u32::MAX)).into(),
            );
            set("backlog", (m.throughput.backlog() as f64).into());
            set("worker_queue", (m.worker_queue as f64).into());
            set("lat_p50_ms", p50.into());
            set("lat_p95_ms", p95.into());
            set("lat_p99_ms", p99.into());
            set("jank_pct", m.jank.jank_pct().into());
            set("worst_frame_ms", m.jank.worst.into());
            web_sys::console::log_2(&"[BENCH]".into(), &dump);
        });
        move || drop(handle)
    });
}

/// Minimal setInterval wrapper returning a guard that clears the interval and
/// drops the closure on drop. (Avoids pulling in gloo-timers for one call.)
struct IntervalHandle {
    id: i32,
    _closure: Closure<dyn FnMut()>,
}
impl Drop for IntervalHandle {
    fn drop(&mut self) {
        if let Some(win) = web_sys::window() {
            win.clear_interval_with_handle(self.id);
        }
    }
}
fn gloo_like_interval(ms: i32, f: impl FnMut() + 'static) -> IntervalHandle {
    let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);
    let id = web_sys::window()
        .and_then(|w| {
            w.set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                ms,
            )
            .ok()
        })
        .unwrap_or(-1);
    IntervalHandle {
        id,
        _closure: closure,
    }
}
