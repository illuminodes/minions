//! The SAB-as-bytes path panel: worker fills a `SharedArrayBuffer` byte ring,
//! the main thread drains it on the animation frame and decodes flat frames.
//!
//! The decode is deliberately NOT a JSON parse — see [`NoteCodec`] for why
//! that distinction is the whole point of this path.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use nostro2::NostrNote;
use nostr_minions::transport::NoteCodec;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::raf::RafLoop;
use crate::sab::SabRing;
use crate::{kind1_filter, metrics, shared, Panel, PathProps, DRAIN_PER_FRAME, FLOOD_SECS, LIMIT};

/// Drains the shared byte ring into the render buffer. Owns the ring handle and
/// the buffer together, so the frames the worker pushes and the notes the panel
/// renders cannot come from different rings.
#[derive(Clone)]
pub struct SabDrain {
    ring: Rc<RefCell<Option<Rc<SabRing>>>>,
    notes: Rc<RefCell<VecDeque<NostrNote>>>,
    metrics: crate::SharedMetrics,
}

impl SabDrain {
    fn new(
        ring: Rc<RefCell<Option<Rc<SabRing>>>>,
        notes: Rc<RefCell<VecDeque<NostrNote>>>,
        metrics: crate::SharedMetrics,
    ) -> Self {
        Self {
            ring,
            notes,
            metrics,
        }
    }

    fn step(&self) -> usize {
        self.metrics.borrow_mut().drain_frames += 1;
        let Some(ring) = self.ring.borrow().clone() else {
            return 0;
        };
        let mut drained = 0usize;
        let mut buf = self.notes.borrow_mut();
        let mut m = self.metrics.borrow_mut();
        for _ in 0..DRAIN_PER_FRAME {
            let Some(frame) = ring.pop() else { break };
            let Some(note) = NoteCodec::decode(&frame) else {
                continue;
            };
            if let Some(em) = metrics::emit_ms_from_content(&note.content) {
                m.latency().record(metrics::wall_ms() - em);
            }
            buf.push_front(note);
            drained += 1;
        }
        buf.truncate(LIMIT as usize);
        if drained > 0 {
            let count = drained as u64;
            m.throughput.produce(count);
            m.throughput.render(count);
            m.dropped = ring.dropped();
        }
        drained
    }
}

#[hook]
fn use_sab_drain(drain: SabDrain) {
    let force = use_force_update();
    use_effect_with((), move |()| {
        let raf = RafLoop::start(move |_ts| {
            if drain.step() > 0 {
                force.force_update();
            }
        });
        move || drop(raf)
    });
}

/// Signals the producer that this panel is gone, so a worker parked on a full
/// ring stops instead of outliving its consumer.
struct ConsumerGuard {
    ring: Rc<RefCell<Option<Rc<SabRing>>>>,
}

impl Drop for ConsumerGuard {
    fn drop(&mut self) {
        if let Some(ring) = self.ring.borrow().as_ref() {
            ring.set_consumer_alive(false);
        }
    }
}

#[function_component(SabPanel)]
pub fn sab_panel(props: &PathProps) -> Html {
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);
    let ring: Rc<RefCell<Option<Rc<SabRing>>>> = use_mut_ref(|| None);
    let status = use_state(|| "ready".to_string());

    use_sab_drain(SabDrain::new(
        ring.clone(),
        notes.clone(),
        props.metrics.clone(),
    ));

    use_effect_with((), {
        let ring = ring.clone();
        move |()| {
            let guard = ConsumerGuard { ring };
            move || drop(guard)
        }
    });

    let flood = {
        let ring = ring.clone();
        let rate = props.rate;
        let payload = props.payload as usize;
        let status = status.clone();
        Callback::from(move |_| {
            if let Some(active) = ring.borrow().as_ref() {
                active.set_consumer_alive(false);
            }
            let fresh = match SabRing::create() {
                Ok(r) => Rc::new(r),
                Err(e) => {
                    status.set(format!("SharedArrayBuffer unavailable: {e:?}"));
                    return;
                }
            };
            fresh.set_consumer_alive(true);
            let buffer = fresh.buffer();
            *ring.borrow_mut() = Some(fresh);

            let Some(glue) = shared::find_glue_url() else {
                status.set("no glue URL".to_string());
                return;
            };
            let filter_json = serde_json::to_string(&kind1_filter()).unwrap_or_default();
            let args = [
                buffer,
                JsValue::from_f64(f64::from(rate)),
                JsValue::from_f64(f64::from(FLOOD_SECS)),
                JsValue::from_f64(payload as f64),
                JsValue::from_str(&filter_json),
            ];
            match shared::spawn_worker(&glue, "sab_worker_main", &args) {
                Ok(worker) => {
                    let status = status.clone();
                    let onmsg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                        if let Some(s) = e.data().as_string() {
                            web_sys::console::log_1(&format!("[sab] {s}").into());
                            status.set(s);
                        }
                    })
                        as Box<dyn FnMut(web_sys::MessageEvent)>);
                    worker.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));
                    onmsg.forget();
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
                {"sab status: "}<code>{ (*status).clone() }</code>
            </p>
            <Panel title="SAB bytes (stable toolchain, flat codec, no postMessage/note)" color="#7c3aed"
                render_count={*renders.borrow()} notes={snapshot} on_flood={flood} />
        </>
    }
}
