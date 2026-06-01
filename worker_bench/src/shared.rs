//! Shared-memory worker handshake + Phase-1 sanity gate.
//!
//! The whole point: the worker thread runs the SAME wasm module against the
//! SAME `WebAssembly.Memory` as the main thread, so a pointer (and the atomics
//! behind it) is valid on both sides. That is what lets a quetzalcoatl ring
//! span the boundary with zero serialization.
//!
//! This module first proves the handshake with a trivial shared `AtomicU32`
//! counter (the gate): the worker increments it in a loop; the main thread
//! reads it climbing. If that works, `crossOriginIsolated` is true and shared
//! memory + atomics are live — everything else (the ring) builds on it.

use std::sync::atomic::{AtomicU32, Ordering};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// A process-lifetime shared counter the worker bumps and the main thread
/// reads. Lives in the shared linear memory; its address is handed to the
/// worker so both sides touch the same atomic.
///
/// `Box::leak`-style permanence: created once, never freed.
///
/// Phase-1 gate scaffolding — kept as the minimal reference handshake (and a
/// debugging fallback) now that Phase 2 uses the real ring. Hence `dead_code`.
#[allow(dead_code)]
pub struct Gate {
    counter: &'static AtomicU32,
}

#[allow(dead_code)] // Phase-1 gate scaffolding; see Gate doc.
impl Gate {
    /// Allocate the shared counter (main thread). The returned pointer (as a
    /// `usize`) is what we hand to the worker.
    #[must_use]
    pub fn new() -> Self {
        let counter: &'static AtomicU32 = Box::leak(Box::new(AtomicU32::new(0)));
        Self { counter }
    }

    /// Address of the shared counter, to pass across to the worker.
    #[must_use]
    pub fn ptr(&self) -> usize {
        std::ptr::from_ref(self.counter) as usize
    }

    /// Current value (main thread reads this climbing).
    #[must_use]
    pub fn value(&self) -> u32 {
        self.counter.load(Ordering::Relaxed)
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

/// Worker entry point — exported to JS so the worker glue can call it after
/// instantiating against the shared memory.
///
/// `counter_ptr` is the address (in the shared linear memory) of the
/// `AtomicU32` allocated by the main thread. The worker reconstructs a
/// reference to it and increments it forever, proving cross-thread shared
/// atomics work.
#[wasm_bindgen]
pub fn worker_gate_main(counter_ptr: usize) {
    // SAFETY: the main thread leaked an AtomicU32 at this address in the shared
    // linear memory; it lives for the program and we only do atomic ops on it.
    let counter: &'static AtomicU32 = unsafe { &*(counter_ptr as *const AtomicU32) };
    // Bump in a tight loop with a tiny yield so we don't peg the worker core.
    // (No sleep API needed for the gate; a spin with periodic yields is fine.)
    loop {
        counter.fetch_add(1, Ordering::Relaxed);
        for _ in 0..50_000 {
            std::hint::spin_loop();
        }
    }
}

/// The worker bootstrap module, embedded in Rust via `link_to!(inline_js = ..)`.
///
/// No separate `.js` file and no hand-built Blob URL: `link_to!` runs this
/// inline module through wasm-bindgen's bundler and yields a proper runtime URL
/// (with a real base), so the `import(glueUrl)` inside it resolves correctly —
/// the exact thing a `blob:` URL couldn't do.
///
/// On the dynamic `import(glueUrl)`: this is deliberate, not lazy. The glue is
/// an ES module (so a classic worker's `importScripts` can't load it), and the
/// glue filename is content-hashed (so a *static* `import` can't name it) — and
/// the wasm-bindgen book forbids static imports inside snippets anyway. The
/// fully clean alternative (wasm-bindgen-rayon's pattern) makes the worker a
/// first-class bundle-graph module with a static import, which requires a JS
/// bundler step Trunk + `link_to!` don't give us. So a dynamic `import()` of an
/// absolute URL (resolved on the main thread, where a document base exists) is
/// the idiomatic ceiling for this toolchain. Revisit only if Trunk grows a
/// shared-memory worker entry. (Verified working; the gate counter climbs.)
///
/// It receives `{ glueUrl, module, memory, entry, args }`, imports the app
/// glue, instantiates THIS worker against the shared module + memory via
/// `initSync`, then calls the named exported entry (`glue[entry](...args)`).
/// Each step is reported back so a failure is visible on the main thread
/// instead of dying silently.
fn worker_boot_url() -> String {
    wasm_bindgen::link_to!(
        inline_js = r#"
self.onmessage = async (e) => {
    const report = (m) => self.postMessage(m);
    try {
        const { glueUrl, module, memory, entry, args } = e.data;
        report("worker: importing glue " + glueUrl);
        const glue = await import(glueUrl);
        report("worker: glue imported, keys=" + Object.keys(glue).join(","));
        report("worker: initSync against shared memory");
        glue.initSync({ module, memory });
        report("worker: calling " + entry + "(" + args.join(",") + ")");
        glue[entry](...args);
        report("worker: entry returned");
    } catch (err) {
        report("worker ERROR: " + (err && err.stack ? err.stack : err));
    }
};
"#
    )
}

/// Spawn a worker that runs the named exported entry against the main thread's
/// shared memory, passing `args` to it.
///
/// `glue_url` is Trunk's `app-<hash>.js`; we resolve it to an ABSOLUTE URL
/// against the document base here (the worker has no document base for its own
/// import resolution). `entry` is the `#[wasm_bindgen]` export to invoke (e.g.
/// `"worker_gate_main"` or `"ring_worker_main"`); `args` are passed positionally
/// (each must be a JS-cloneable `JsValue` — numbers, strings, etc.).
///
/// # Errors
/// Returns a `JsValue` if Worker construction or postMessage fails.
pub fn spawn_worker(
    glue_url: &str,
    entry: &str,
    args: &[JsValue],
) -> Result<web_sys::Worker, JsValue> {
    let base = web_sys::window()
        .and_then(|w| w.location().href().ok())
        .unwrap_or_default();
    let glue_abs = web_sys::Url::new_with_base(glue_url, &base)
        .map(|u| u.href())
        .unwrap_or_else(|_| glue_url.to_string());

    let opts = web_sys::WorkerOptions::new();
    opts.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&worker_boot_url(), &opts)?;

    let arg_arr = web_sys::js_sys::Array::new();
    for a in args {
        arg_arr.push(a);
    }
    let msg = web_sys::js_sys::Object::new();
    web_sys::js_sys::Reflect::set(&msg, &"glueUrl".into(), &JsValue::from_str(&glue_abs))?;
    web_sys::js_sys::Reflect::set(&msg, &"module".into(), &wasm_bindgen::module())?;
    web_sys::js_sys::Reflect::set(&msg, &"memory".into(), &wasm_bindgen::memory())?;
    web_sys::js_sys::Reflect::set(&msg, &"entry".into(), &JsValue::from_str(entry))?;
    web_sys::js_sys::Reflect::set(&msg, &"args".into(), &arg_arr)?;
    worker.post_message(&msg)?;

    Ok(worker)
}

/// Convenience wrapper for the Phase-1 gate (calls `worker_gate_main(ptr)`).
///
/// # Errors
/// Returns a `JsValue` if Worker construction or postMessage fails.
#[allow(dead_code)] // Phase-1 gate convenience; superseded by the ring path.
pub fn spawn_gate_worker(glue_url: &str, counter_ptr: usize) -> Result<web_sys::Worker, JsValue> {
    spawn_worker(
        glue_url,
        "worker_gate_main",
        &[JsValue::from_f64(counter_ptr as f64)],
    )
}

/// Read the Trunk-emitted glue URL from the running document.
///
/// Trunk injects `<link rel="modulepreload" href="/app-<hash>.js">` and an
/// inline module that imports the same; we find the app glue URL by scanning
/// the document's scripts/links for the `app-` glue. Returns `None` if not
/// found (e.g. running under a different bundler).
#[must_use]
pub fn find_glue_url() -> Option<String> {
    let doc = web_sys::window()?.document()?;
    let links = doc.query_selector_all("link[rel=modulepreload]").ok()?;
    for i in 0..links.length() {
        let Some(node) = links.item(i) else { continue };
        let Ok(el) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        if let Some(href) = el.get_attribute("href") {
            if href.contains("/app-") && href.ends_with(".js") {
                return Some(href);
            }
        }
    }
    None
}
