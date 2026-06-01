//! Three-way relay-ingestion benchmark, instrumented identically across paths:
//!
//! - **In-thread**: parse + dedup + filter on the UI thread (what the
//!   `nostr-minions` pool does in its socket `onmessage`). Flood feeder runs
//!   that work on the main thread, in coarse bursts (like a real socket).
//! - **Reactor (postMessage)**: a `yew-agent` worker does the work off-thread
//!   and streams matched notes back across the bridge (JSON encode +
//!   postMessage per note).
//! - **Shared ring (SharedArrayBuffer)**: a hand-spawned worker shares the wasm
//!   linear memory and pushes matched `NostrNote`s into a `quetzalcoatl` SPSC
//!   ring; the main thread pops them. Zero serialization, zero per-note
//!   postMessage (only a one-time startup handshake).
//!
//! Metrics (see `metrics.rs`): main-thread jank histogram, throughput
//! (produced/rendered per sec + backlog), per-note end-to-end latency
//! (p50/p95/p99, via a timestamp embedded in each synthetic note's content so
//! it survives the boundary), and a 1 Hz `[BENCH]` console dump. The
//! `SmoothnessMeter` is the human-visible version of the jank histogram.

#[path = "relay_worker.rs"]
mod relay_worker;
mod ring;
mod shared;
// Reuse the single metrics module loaded by relay_worker; loading metrics.rs
// again via #[path] would define it twice in this bin.
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
/// Max notes drained per animation frame (reactor + ring), so a fast stream
/// can't block the UI in one synchronous burst — the rest waits for next frame.
const DRAIN_PER_FRAME: usize = 200;

fn kind1_filter() -> NostrSubscription {
    NostrSubscription {
        kinds: Some(vec![1]),
        limit: Some(LIMIT),
        ..Default::default()
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("panic: {info}").into());
    }));
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
    /// Worker-side bridge queue depth (reactor path only): notes matched but not
    /// yet shipped across the postMessage bridge — the hidden backlog the app's
    /// own metric can't see. 0 for in-thread and ring paths.
    worker_queue: u64,
    /// Notes dropped because the shared ring was full (ring path only). Honest
    /// overload signal: the ring is bounded, so when the consumer can't keep up
    /// the producer drops rather than growing memory unbounded.
    dropped: u64,
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
        self.dropped = 0;
    }
}

type SharedMetrics = Rc<RefCell<Metrics>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Path {
    None,
    InThread,
    Reactor,
    Ring,
}

#[function_component(App)]
fn app() -> Html {
    let path = use_state(|| Path::None);
    let rate = use_state(|| 2000u32);
    // Extra bytes padded into each note's content — the message-SIZE axis.
    let payload = use_state(|| 0u32);
    let metrics: SharedMetrics = use_mut_ref(Metrics::default);

    let isolated = web_sys::window()
        .and_then(|w| {
            web_sys::js_sys::Reflect::get(&w, &"crossOriginIsolated".into())
                .ok()
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false);

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

    let iso_color = if isolated { "#16a34a" } else { "#dc2626" };
    html! {
        <div style="font-family:system-ui; padding:1rem; background:#f3f4f6; min-height:100vh;">
            <h1 style="text-align:center; margin:.2rem;">{"Relay Pool Benchmark — 3 architectures"}</h1>
            <p style="text-align:center; font-size:.8rem; color:#6b7280; margin:.2rem;">
                {"One path at a time. Same synthetic flood through the same parse+dedup+filter for all three. "}
                {"crossOriginIsolated: "}<b style={format!("color:{iso_color}")}>{ if isolated {"true"} else {"false"} }</b>
                {" (shared-ring path needs true)"}
            </p>

            <div style="text-align:center; margin:.5rem;">
                { btn("In-Thread", Path::InThread, switch(Path::InThread, "in-thread"), "#2563eb") }
                { btn("Reactor (postMessage)", Path::Reactor, switch(Path::Reactor, "reactor"), "#d97706") }
                { btn("Shared Ring (SAB)", Path::Ring, switch(Path::Ring, "ring"), "#16a34a") }
                { btn("Stop", Path::None, switch(Path::None, "idle"), "#6b7280") }
                <span style="margin-left:1rem; font-size:.85rem;">
                    {"rate: "}
                    <input type="number" min="100" max="50000" step="100"
                        value={rate.to_string()} oninput={on_rate}
                        style="width:6rem; padding:.2rem;" />
                </span>
                <span style="margin-left:1rem; font-size:.85rem;">
                    {"payload bytes: "}
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
                            {"Pick a path, then click Flood inside it. Compare jank %, latency, and the smoothness FPS across all three."}
                        </p>
                    },
                    Path::InThread => html!{
                        <InThreadPanel key="in-thread" metrics={metrics.clone()} rate={*rate} payload={*payload} />
                    },
                    Path::Reactor => html!{
                        <ReactorProvider<RelayReactor, JsonCodec> key="reactor" path="/worker.js">
                            <ReactorPanel metrics={metrics.clone()} rate={*rate} payload={*payload} />
                        </ReactorProvider<RelayReactor, JsonCodec>>
                    },
                    Path::Ring => html!{
                        <RingPanel key="ring" metrics={metrics.clone()} rate={*rate} payload={*payload} />
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

/// In-thread path: flood feeder runs parse + dedup + filter ON THE MAIN THREAD
/// in coarse non-yielding bursts (like a real socket `onmessage`), then renders.
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
                const BURST_MS: u32 = 100;
                let bursts = FLOOD_SECS * (1000 / BURST_MS);
                let per_burst = (rate * BURST_MS / 1000).max(1);
                for _ in 0..bursts {
                    let emit = metrics::wall_ms();
                    let mut produced = 0u64;
                    let mut rendered = 0u64;
                    for _ in 0..per_burst {
                        let s = *seq.borrow();
                        *seq.borrow_mut() += 1;
                        produced += 1;
                        let raw = metrics::synthetic_event(s, emit, payload);
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

/// Reactor path: flood runs in a yew-agent worker; matched notes stream back
/// over the postMessage bridge (JSON-coded per note).
#[function_component(ReactorPanel)]
fn reactor_panel(props: &PathProps) -> Html {
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);
    let seq = use_mut_ref(|| 1u64);
    let pending = use_mut_ref(VecDeque::<NostrNote>::new);

    let bridge = {
        let pending = pending.clone();
        let metrics = props.metrics.clone();
        use_reactor_bridge::<RelayReactor, _>(move |ev| {
            if let yew_agent::reactor::ReactorEvent::Output(out) = ev {
                match out {
                    relay_worker::WorkerOut::Note(note) => {
                        if let Some(em) = metrics::emit_ms_from_content(&note.content) {
                            metrics
                                .borrow_mut()
                                .latency()
                                .record(metrics::wall_ms() - em);
                        }
                        metrics.borrow_mut().throughput.produce(1);
                        pending.borrow_mut().push_back(note);
                    }
                    relay_worker::WorkerOut::QueueDepth(depth) => {
                        metrics.borrow_mut().worker_queue = depth;
                    }
                }
            }
        })
    };

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

    drain_loop(notes.clone(), pending, props.metrics.clone());

    *renders.borrow_mut() += 1;
    let snapshot: Vec<NostrNote> = notes.borrow().iter().cloned().collect();
    html! {
        <Panel title="Reactor (postMessage bridge)" color="#d97706"
            render_count={*renders.borrow()} notes={snapshot} on_flood={flood} />
    }
}

/// Shared-ring path: a hand-spawned worker shares the wasm memory and pushes
/// matched notes into a quetzalcoatl SPSC ring; we pop from it. Zero
/// serialization, no per-note postMessage.
#[function_component(RingPanel)]
fn ring_panel(props: &PathProps) -> Html {
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);
    let ring_ptr = use_mut_ref(|| 0usize);
    let dropped_ptr = use_mut_ref(|| 0usize);
    // Shared "flood running" flag address. We gate new floods on it so the SPSC
    // ring never has two concurrent producers. We deliberately do NOT keep a
    // worker handle to terminate(): hard-killing a thread that shares the wasm
    // allocator can orphan the dlmalloc lock mid-`malloc` and hang every later
    // allocation ("page unresponsive"). Each flood worker exits cooperatively
    // after its run and clears the flag itself.
    let running_ptr = use_mut_ref(|| 0usize);
    let status = use_state(|| "ready".to_string());

    // rAF loop: lazily build the consumer once the ring exists, then pop up to
    // DRAIN_PER_FRAME notes/frame into the view, recording latency + throughput.
    {
        let notes = notes.clone();
        let ring_ptr = ring_ptr.clone();
        let dropped_ptr = dropped_ptr.clone();
        let metrics = props.metrics.clone();
        let force = use_force_update();
        use_effect_with((), move |()| {
            // The consumer is bound to ONE ring pointer. Each flood allocates a
            // FRESH ring (see `flood`), so when `ring_ptr` changes we must drop
            // the old consumer and build a new one against the new ring. A
            // quetzalcoatl `Producer` starts its write cursor at 0 regardless of
            // the ring's current `tail`, so reusing a ring across floods would
            // rewind `tail` under a consumer whose `head` is still at the old
            // high-water mark — the consumer would then read uninitialized slots
            // (`assume_init_read` on stale memory), producing `NostrNote`s with
            // garbage `String` pointers and corrupting the shared dlmalloc heap.
            // That heap corruption is the "page unresponsive on the 2nd flood".
            // Binding the consumer to the live pointer makes each flood a clean,
            // fresh producer/consumer pair on its own ring.
            let consumer: Rc<RefCell<Option<_>>> = Rc::new(RefCell::new(None));
            let bound_ptr = Rc::new(RefCell::new(0usize));
            let cb: RafClosure = Rc::new(RefCell::new(None));
            let cb2 = cb.clone();
            *cb.borrow_mut() = Some(Closure::wrap(Box::new(move |_ts: f64| {
                let ptr = *ring_ptr.borrow();
                // Reflect the worker's shared drop counter into metrics each frame.
                let dropped = unsafe { ring::dropped_count(*dropped_ptr.borrow()) };
                if dropped > 0 {
                    metrics.borrow_mut().dropped = dropped;
                }
                if ptr != 0 {
                    if *bound_ptr.borrow() != ptr {
                        // New ring (first flood, or a fresh ring for this flood):
                        // (re)build the consumer against the current pointer.
                        // SAFETY: ptr from ring::alloc_ring (leaked, 'static);
                        // single consumer (here), single producer (worker).
                        *consumer.borrow_mut() = Some(unsafe { ring::consumer(ptr) });
                        *bound_ptr.borrow_mut() = ptr;
                    }
                    let mut drained = 0u64;
                    if let Some(c) = consumer.borrow_mut().as_mut() {
                        let mut buf = notes.borrow_mut();
                        let mut m = metrics.borrow_mut();
                        for _ in 0..DRAIN_PER_FRAME {
                            let Some(note) = c.pop() else { break };
                            if let Some(em) = metrics::emit_ms_from_content(&note.content) {
                                m.latency().record(metrics::wall_ms() - em);
                            }
                            buf.push_front(note);
                            drained += 1;
                        }
                        buf.truncate(LIMIT as usize);
                    }
                    if drained > 0 {
                        let mut m = metrics.borrow_mut();
                        // worker pushed = we popped (it dropped non-matches itself).
                        m.throughput.produce(drained);
                        m.throughput.render(drained);
                        drop(m);
                        force.force_update();
                    }
                }
                if let (Some(win), Some(c)) = (web_sys::window(), cb2.borrow().as_ref()) {
                    let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut(f64)>));
            if let (Some(win), Some(c)) = (web_sys::window(), cb.borrow().as_ref()) {
                let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
            }
            // Stop the self-rescheduling rAF chain on unmount: clear the cell so
            // the next already-scheduled frame finds `None` and does NOT
            // reschedule. Just dropping the outer `cb` would NOT cancel the
            // in-flight frame (the browser still holds `cb2`), leaking an
            // immortal loop per mount — which compounded across path switches
            // and exploded after a tab background/resume requeued them.
            move || {
                *cb.borrow_mut() = None;
            }
        });
    }

    let flood = {
        let ring_ptr = ring_ptr.clone();
        let dropped_ptr = dropped_ptr.clone();
        let running_ptr = running_ptr.clone();
        let rate = props.rate;
        let payload = props.payload as usize;
        let status = status.clone();
        Callback::from(move |_| {
            web_sys::console::log_1(&"[ring] flood click: start".into());
            // Refuse to start a second flood while one is running — two
            // producers would violate the SPSC ring's single-producer contract.
            if unsafe { ring::is_running(*running_ptr.borrow()) } {
                status.set("flood already running — wait for it to finish".to_string());
                return;
            }
            // Allocate a FRESH ring per flood. A quetzalcoatl `Producer` always
            // starts its write cursor at 0, ignoring the ring's existing `tail`;
            // reusing a ring across floods would rewind `tail` under a consumer
            // whose `head` is at the previous high-water mark, so the consumer
            // would read uninitialized slots and corrupt the shared heap (the
            // 2nd-flood "page unresponsive"). The old ring is `Box::leak`ed
            // (process-lifetime) and simply abandoned — no double-producer, and
            // the rAF loop rebinds its consumer to this new pointer.
            web_sys::console::log_1(&"[ring] alloc_ring…".into());
            let (rp, dp, runp) = ring::alloc_ring();
            web_sys::console::log_1(&"[ring] alloc_ring done".into());
            *ring_ptr.borrow_mut() = rp;
            *dropped_ptr.borrow_mut() = dp;
            *running_ptr.borrow_mut() = runp;
            let ptr = *ring_ptr.borrow();
            let dptr = *dropped_ptr.borrow();
            let runp = *running_ptr.borrow();
            let Some(glue) = shared::find_glue_url() else {
                status.set("no glue URL".to_string());
                return;
            };
            let filter_json = serde_json::to_string(&kind1_filter()).unwrap_or_default();
            let args = [
                JsValue::from_f64(ptr as f64),
                JsValue::from_f64(dptr as f64),
                JsValue::from_f64(runp as f64),
                JsValue::from_f64(f64::from(rate)),
                JsValue::from_f64(f64::from(FLOOD_SECS)),
                JsValue::from_f64(payload as f64),
                JsValue::from_str(&filter_json),
            ];
            match shared::spawn_worker(&glue, "ring_worker_main", &args) {
                Ok(worker) => {
                    let status = status.clone();
                    let onmsg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                        if let Some(s) = e.data().as_string() {
                            web_sys::console::log_1(&format!("[ring] {s}").into());
                            status.set(s);
                        }
                    })
                        as Box<dyn FnMut(web_sys::MessageEvent)>);
                    worker.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));
                    onmsg.forget();
                    // The worker exits cooperatively after its run (clearing the
                    // running flag); we never terminate() it. Forgetting it here
                    // is fine — it's a one-shot flood that ends on its own, and a
                    // dead worker frees itself. (One worker per flood; gated by
                    // the running flag so they never overlap.)
                    std::mem::forget(worker);
                }
                Err(e) => status.set(format!("spawn failed: {e:?}")),
            }
        })
    };

    *renders.borrow_mut() += 1;
    let snapshot: Vec<NostrNote> = notes.borrow().iter().cloned().collect();
    html! {
        <>
            <p style="text-align:center; font-size:.75rem; color:#6b7280; margin:.2rem;">
                {"ring status: "}<code>{ (*status).clone() }</code>
            </p>
            <Panel title="Shared Ring (zero-copy, no postMessage)" color="#16a34a"
                render_count={*renders.borrow()} notes={snapshot} on_flood={flood} />
        </>
    }
}

/// Shared rAF drain loop for the reactor path: move up to DRAIN_PER_FRAME
/// pending notes into the render buffer, render once per frame when non-empty.
fn drain_loop(
    notes: Rc<RefCell<VecDeque<NostrNote>>>,
    pending: Rc<RefCell<VecDeque<NostrNote>>>,
    metrics: SharedMetrics,
) {
    #[hook]
    fn use_drain(
        notes: Rc<RefCell<VecDeque<NostrNote>>>,
        pending: Rc<RefCell<VecDeque<NostrNote>>>,
        metrics: SharedMetrics,
    ) {
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
            // Stop the self-rescheduling rAF chain on unmount: clear the cell so
            // the next already-scheduled frame finds `None` and does NOT
            // reschedule. Just dropping the outer `cb` would NOT cancel the
            // in-flight frame (the browser still holds `cb2`), leaking an
            // immortal loop per mount — which compounded across path switches
            // and exploded after a tab background/resume requeued them.
            move || {
                *cb.borrow_mut() = None;
            }
        });
    }
    use_drain(notes, pending, metrics);
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
        <div style="background:white; border-radius:8px; padding:1rem; box-shadow:0 1px 3px rgba(0,0,0,.1); display:flex; flex-direction:column; height:52vh;">
            <div style="display:flex; justify-content:space-between; align-items:center;">
                <h2 style={format!("color:{}; margin:0; font-size:1.05rem;", props.color)}>{props.title}</h2>
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
                { cell("dropped", m.dropped.to_string(),
                    if m.dropped > 0 { "#dc2626" } else { "#111827" }) }
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

/// Main-thread-driven smoothness indicator: a rAF-spun bar + rolling FPS. It
/// freezes when the UI thread blocks (in-thread bursts) and stays smooth when
/// work is off-thread (reactor / ring). Human-visible version of jank %.
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
            // Stop the self-rescheduling rAF chain on unmount: clear the cell so
            // the next already-scheduled frame finds `None` and does NOT
            // reschedule. Just dropping the outer `cb` would NOT cancel the
            // in-flight frame (the browser still holds `cb2`), leaking an
            // immortal loop per mount — which compounded across path switches
            // and exploded after a tab background/resume requeued them.
            move || {
                *cb.borrow_mut() = None;
            }
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
            // Stop the self-rescheduling rAF chain on unmount: clear the cell so
            // the next already-scheduled frame finds `None` and does NOT
            // reschedule. Just dropping the outer `cb` would NOT cancel the
            // in-flight frame (the browser still holds `cb2`), leaking an
            // immortal loop per mount — which compounded across path switches
            // and exploded after a tab background/resume requeued them.
            move || {
                *cb.borrow_mut() = None;
            }
        });
    }

    use_effect_with((), move |()| {
        let handle = gloo_like_interval(1000, move || {
            let mut m = metrics.borrow_mut();
            m.throughput.sample();
            let (p50, p95, p99) = m
                .latency_samples
                .as_ref()
                .map_or((0.0, 0.0, 0.0), Latency::percentiles);
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
            set("dropped", (m.dropped as f64).into());
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

/// Minimal `setInterval` wrapper; clears the interval and drops the closure on
/// drop. (Avoids pulling in gloo-timers for one call.)
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
