//! Benchmark application (UI thread / `data-type="main"`).
//!
//! Renders two panels side by side, each subscribed to the SAME relays and the
//! SAME kind-1 filter, so the only difference is WHERE the work happens:
//!
//! - In-thread panel: `nostr-minions` pool. JSON parse + dedup + filter all run
//!   on the UI thread.
//! - Worker panel: `RelayReactor` in a Web Worker. That work runs off the UI
//!   thread; matched notes cross the bridge (bincode + postMessage) before
//!   rendering.
//!
//! Each panel shows its render count and notes-received count. A shared
//! main-thread "jank meter" samples requestAnimationFrame deltas so you can
//! watch UI-thread responsiveness while notes stream in.

#[path = "relay_worker.rs"]
mod relay_worker;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use nostro2::{NostrNote, NostrSubscription};
use relay_worker::{RelayCommand, RelayReactor};
use yew::prelude::*;
use yew_agent::reactor::{use_reactor_subscription, ReactorProvider};

const RELAYS: [&str; 2] = ["wss://relay.damus.io", "wss://relay.nostr.band"];
const LIMIT: u32 = 50;

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

#[function_component(App)]
fn app() -> Html {
    html! {
        <div style="font-family: system-ui; padding: 1rem; background:#f3f4f6; min-height:100vh;">
            <h1 style="text-align:center;">{"Relay Pool: In-Thread vs Web Worker"}</h1>
            <JankMeter />
            <div style="display:flex; gap:1rem; align-items:flex-start;">
                <div style="flex:1; min-width:0;">
                    // In-thread path uses nostr-minions' own provider stack.
                    <nostr_minions::NostrAppProvider
                        relays={RELAYS.iter().map(|u| nostr_minions::UserRelay{
                            url: (*u).to_string(), read:true, write:true
                        }).collect::<Vec<_>>()}
                        fallback={html!(<p>{"loading…"}</p>)}
                    >
                        <InThreadPanel />
                    </nostr_minions::NostrAppProvider>
                </div>
                <div style="flex:1; min-width:0;">
                    // Worker path: reactor provider points at the worker bundle.
                    <ReactorProvider<RelayReactor> path="/worker.js">
                        <WorkerPanel />
                    </ReactorProvider<RelayReactor>>
                </div>
            </div>
        </div>
    }
}

/// Panel backed by the in-thread `nostr-minions` pool.
#[function_component(InThreadPanel)]
fn in_thread_panel() -> Html {
    let notes = nostr_minions::use_text_notes(Some(LIMIT));
    let renders = use_mut_ref(|| 0usize);
    *renders.borrow_mut() += 1;
    let render_count = *renders.borrow();

    html! {
        <Panel
            title="In-Thread (nostr-minions)"
            color="#2563eb"
            render_count={render_count}
            notes={notes}
        />
    }
}

/// Panel backed by the worker `RelayReactor`.
#[function_component(WorkerPanel)]
fn worker_panel() -> Html {
    // Bridge to the worker. The handle is a stream of NostrNote outputs and a
    // sink for RelayCommand inputs.
    let sub = use_reactor_subscription::<RelayReactor>();

    // Accumulate streamed notes locally (most-recent-first, bounded by LIMIT).
    let notes = use_mut_ref(VecDeque::<NostrNote>::new);
    let renders = use_mut_ref(|| 0usize);

    // On mount: tell the worker which relays to connect and what to subscribe.
    use_effect_with((), {
        let sub = sub.clone();
        move |()| {
            sub.send(RelayCommand::Connect(
                RELAYS.iter().map(|u| (*u).to_string()).collect(),
            ));
            sub.send(RelayCommand::Subscribe(kind1_filter()));
            || ()
        }
    });

    // Drain newly streamed notes from the subscription into our buffer.
    // `sub` collects outputs into a slice; we mirror the tail we haven't seen.
    {
        let notes = notes.clone();
        use_effect_with(sub.len(), move |_| {
            // The subscription exposes received outputs as an indexable slice.
            // Rebuild our bounded, newest-first view from it.
            let mut buf = notes.borrow_mut();
            buf.clear();
            for note in sub.iter().rev().take(LIMIT as usize) {
                buf.push_back((**note).clone());
            }
            || ()
        });
    }

    *renders.borrow_mut() += 1;
    let render_count = *renders.borrow();
    let snapshot: Vec<NostrNote> = notes.borrow().iter().cloned().collect();

    html! {
        <Panel
            title="Web Worker (RelayReactor)"
            color="#16a34a"
            render_count={render_count}
            notes={snapshot}
        />
    }
}

#[derive(Properties, PartialEq)]
struct PanelProps {
    title: &'static str,
    color: &'static str,
    render_count: usize,
    notes: Vec<NostrNote>,
}

/// Shared presentation for both panels so the only measured difference is the
/// data source, not the rendering work.
#[function_component(Panel)]
fn panel(props: &PanelProps) -> Html {
    html! {
        <div style="background:white; border-radius:8px; padding:1rem; box-shadow:0 1px 3px rgba(0,0,0,.1); display:flex; flex-direction:column; height:70vh;">
            <div style="display:flex; justify-content:space-between; align-items:baseline;">
                <h2 style={format!("color:{}; margin:0;", props.color)}>{props.title}</h2>
                <div style="text-align:right; font-size:.8rem; color:#6b7280;">
                    <div>{format!("renders: {}", props.render_count)}</div>
                    <div style={format!("color:{}; font-weight:600;", props.color)}>
                        {format!("notes: {}", props.notes.len())}
                    </div>
                </div>
            </div>
            <div style="overflow-y:auto; margin-top:.5rem; flex:1; min-height:0;">
                { if props.notes.is_empty() {
                    html!{ <p style="color:#9ca3af; font-style:italic;">{"waiting for notes…"}</p> }
                } else {
                    html!{ <> { for props.notes.iter().map(render_note) } </> }
                }}
            </div>
        </div>
    }
}

fn render_note(note: &NostrNote) -> Html {
    let from = note.pubkey.get(..12).unwrap_or(&note.pubkey);
    html! {
        <div style="padding:.5rem; background:#f9fafb; border:1px solid #e5e7eb; border-radius:6px; margin-bottom:.4rem;">
            <div style="font-size:.7rem; color:#6b7280; font-family:monospace;">{format!("{from}…")}</div>
            <div style="font-size:.85rem; color:#111827; overflow:hidden; text-overflow:ellipsis;">
                { note.content.chars().take(140).collect::<String>() }
            </div>
        </div>
    }
}

/// Samples requestAnimationFrame deltas to expose main-thread jank. A smooth
/// 60fps thread holds ~16.7ms; spikes mean the UI thread is blocked (e.g. by
/// JSON parsing a burst of relay messages in the in-thread path).
#[function_component(JankMeter)]
fn jank_meter() -> Html {
    let worst = use_mut_ref(|| 0f64);
    let last = use_mut_ref(|| 0f64);
    let display = use_state(|| (0f64, 0f64)); // (last delta, worst delta)

    use_effect_with((), {
        let worst = worst.clone();
        let last = last.clone();
        let display = display.clone();
        move |()| {
            // Self-rescheduling rAF loop measuring frame intervals.
            let cb: RafClosure = Rc::new(RefCell::new(None));
            let cb2 = cb.clone();
            *cb.borrow_mut() = Some(Closure::wrap(Box::new(move |ts: f64| {
                let prev = *last.borrow();
                if prev > 0.0 {
                    let delta = ts - prev;
                    if delta > *worst.borrow() {
                        *worst.borrow_mut() = delta;
                    }
                    display.set((delta, *worst.borrow()));
                }
                *last.borrow_mut() = ts;
                if let Some(window) = web_sys::window() {
                    if let Some(closure) = cb2.borrow().as_ref() {
                        let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
                    }
                }
            }) as Box<dyn FnMut(f64)>));
            if let Some(window) = web_sys::window() {
                if let Some(closure) = cb.borrow().as_ref() {
                    let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
                }
            }
            // Keep the closure alive for the lifetime of the component.
            move || drop(cb)
        }
    });

    let (delta, worst_v) = *display;
    html! {
        <div style="text-align:center; margin-bottom:1rem; font-family:monospace; font-size:.85rem;">
            <span style="color:#6b7280;">{"main-thread frame: "}</span>
            <span style="font-weight:700;">{format!("{delta:.1}ms")}</span>
            <span style="color:#6b7280;">{"  worst: "}</span>
            <span style="font-weight:700; color:#dc2626;">{format!("{worst_v:.1}ms")}</span>
        </div>
    }
}

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Self-rescheduling `requestAnimationFrame` callback handle.
type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
