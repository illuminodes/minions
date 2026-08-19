//! Self-spawning the relay worker from the SAME wasm module — no second cargo
//! bin, no hand-written `worker.js`, no `index.html` change for consumers.
//!
//! ## How
//!
//! `yew-agent`'s [`ReactorSpawner`] normally wants a `path` to a separately
//! built worker script. We instead drive the spawner with `with_loader(true)`,
//! which makes it use our URL **verbatim** as the worker's module. That URL is a
//! runtime `Blob` whose JS:
//!
//! 1. dynamically `import()`s this app's own wasm-bindgen glue (Trunk's
//!    `app-<hash>.js`, discovered from the document on the main thread and
//!    resolved to an absolute URL — the worker has no document base of its own),
//! 2. runs a **normal** `init(glueUrl)` so the worker gets its OWN fresh
//!    `WebAssembly.Memory` (no `SharedArrayBuffer`, no nightly — that's the SAB
//!    path we deliberately dropped), then
//! 3. calls [`relay_worker_main`], the `#[wasm_bindgen]` export that registers
//!    the [`RelayReactor`](super::worker::RelayReactor) on the worker side.
//!
//! After step 3 the reactor's registrar installs the `onmessage` handler and
//! posts `WorkerLoaded`; `yew-agent`'s own JSON `postMessage` handshake takes
//! over from there. Nothing about the wasm module or memory is shared across the
//! boundary — only serialized [`RelayCommand`]/[`WorkerOut`] messages.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew_agent::reactor::ReactorBridge;
use yew_agent::Spawnable;

use super::worker::{JsonCodec, RelayReactor};

/// Worker-side entry: register the relay reactor, then offer the shared rings.
///
/// Exported to JS so the boot Blob (below) can call it after `init()`. Runs on
/// the worker thread only; the main thread never calls it.
///
/// The WORKER allocates the rings because `yew-agent` keeps its
/// `web_sys::Worker` private, leaving the application no object to post a
/// `SharedArrayBuffer` on. Here the global scope is always reachable.
///
/// The order below is a correctness requirement, not a preference. The
/// application decodes the handshake as `WorkerLoaded`, which flushes its
/// pending queue to this worker. The registrar's `onmessage` must already
/// exist, or those messages arrive with no listener and are lost.
#[wasm_bindgen]
pub fn relay_worker_main() {
    use yew_agent::Registrable;
    RelayReactor::registrar().encoding::<JsonCodec>().register();
    WorkerRings::offer();
}

/// The worker's half of the ring handshake.
pub struct WorkerRings;

impl WorkerRings {
    thread_local! {
        static TRANSPORT: std::rc::Rc<super::pool_transport::PoolTransport> =
            std::rc::Rc::new(super::pool_transport::PoolTransport::for_worker_owned());
    }

    /// Build the transport and post the handles to the application.
    ///
    /// A worker that cannot allocate rings simply posts nothing; the
    /// application then keeps using the bridge, so both sides still agree.
    fn offer() {
        let transport = Self::transport();
        let Some(handles) = transport.worker_handles() else {
            return;
        };
        let frame = super::handshake::RingHandshake::frame(&handles);
        if let Ok(scope) =
            web_sys::js_sys::global().dyn_into::<web_sys::DedicatedWorkerGlobalScope>()
        {
            let _ = scope.post_message(&frame);
        }
    }

    /// This worker's transport, built once.
    pub fn transport() -> std::rc::Rc<super::pool_transport::PoolTransport> {
        Self::TRANSPORT.with(std::clone::Clone::clone)
    }
}

/// Whether the current thread is the self-spawned relay worker.
///
/// The relay pool reuses the app's own wasm module for its background worker
/// (no separate bin). That means the worker's wasm `init` runs the app's entry
/// point too — which would try to render the UI on a thread that has no DOM and
/// panic. Guard the very top of your app's entry with this so the worker thread
/// skips rendering and only runs the relay reactor:
///
/// ```ignore
/// fn main() {
///     if nostr_minions::is_relay_worker() {
///         return; // worker thread: the reactor registers itself, no UI here
///     }
///     yew::Renderer::<App>::new().render();
/// }
/// ```
///
/// Detected via a flag the boot module sets on the worker's global scope before
/// `init` runs, so it is reliable even though both threads share one module.
#[must_use]
pub fn is_relay_worker() -> bool {
    // `window()` is `None` in any worker; the boot module additionally stamps a
    // `__NOSTR_MINIONS_RELAY_WORKER` flag on the worker global so this is
    // specific to OUR worker (not some unrelated worker the app may spawn).
    use web_sys::js_sys;
    js_sys::global()
        .dyn_into::<web_sys::DedicatedWorkerGlobalScope>()
        .ok()
        .and_then(|scope| {
            js_sys::Reflect::get(&scope, &JsValue::from_str("__NOSTR_MINIONS_RELAY_WORKER")).ok()
        })
        .is_some_and(|v| v.is_truthy())
}

/// Build the worker boot module as a runtime `Blob` URL with `glue_abs` and
/// `wasm_abs` (ABSOLUTE URLs to this app's wasm-bindgen glue and its `_bg.wasm`)
/// baked in.
///
/// The JS is an ES module (`type: "module"` worker): it dynamically imports the
/// glue and `await`s `init({{ module_or_path: wasm_abs }})`. Passing the wasm URL
/// EXPLICITLY is essential — the module runs from a `blob:` URL, so `init`'s
/// default `import.meta.url`-relative resolution would resolve `_bg.wasm`
/// against the blob (no real base) and fetch the dev server's `index.html`
/// instead, failing with a Wasm "magic word" compile error. After init it calls
/// the exported `relay_worker_main` (the reactor registrar). A `blob:` URL is
/// fine because BOTH the `import()` and the wasm path are absolute.
fn worker_boot_url(glue_abs: &str, wasm_abs: &str) -> Result<String, JsValue> {
    let js = format!(
        r#"
// Mark this worker BEFORE init runs: `init()` executes the app's wasm start
// function (the consumer's entry point), which calls `is_relay_worker()` to
// decide whether to skip rendering. The flag must already be set by then.
self.__NOSTR_MINIONS_RELAY_WORKER = true;
import init, {{ relay_worker_main }} from "{glue_abs}";
try {{
    await init({{ module_or_path: "{wasm_abs}" }});
    relay_worker_main();
}} catch (err) {{
    self.postMessage("relay worker boot ERROR: " + (err && err.stack ? err.stack : err));
}}
"#
    );

    let parts = web_sys::js_sys::Array::new();
    parts.push(&JsValue::from_str(&js));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts)?;
    web_sys::Url::create_object_url_with_blob(&blob)
}

/// Discover this app's wasm-bindgen glue + `_bg.wasm` URLs from the running
/// document, resolved to ABSOLUTE URLs.
///
/// Returns `(glue_abs, wasm_abs)`. The filenames are the bin name + content hash
/// (e.g. `myapp-<hash>.js` / `myapp-<hash>_bg.wasm`), unknowable ahead of time.
/// The bin-name-agnostic anchor is the wasm-bindgen naming convention: glue
/// `<name>.js` always has a sibling `<name>_bg.wasm`. Trunk preloads the wasm as
/// `<link rel="preload" as="fetch" type="application/wasm" href="…_bg.wasm">`, so
/// we read that href (→ wasm) and map `_bg.wasm` → `.js` (→ glue). We fall back
/// to scanning `modulepreload` links for any `.js` (→ glue) and deriving the
/// wasm by the inverse mapping. Both are resolved against the document base so
/// the worker — which has no base of its own — can load them. Returns `None`
/// under a bundler that emits neither (the relay pool then stays unconnected
/// rather than panicking).
fn find_module_urls() -> Option<(String, String)> {
    let window = web_sys::window()?;
    let doc = window.document()?;
    let base = window.location().href().ok()?;
    let abs = |href: &str| {
        web_sys::Url::new_with_base(href, &base)
            .map(|u| u.href())
            .ok()
    };

    // Primary: the wasm preload link → wasm directly, glue by `_bg.wasm` → `.js`.
    if let Ok(Some(node)) = doc.query_selector("link[rel=preload][type='application/wasm']") {
        if let Ok(el) = node.dyn_into::<web_sys::Element>() {
            if let Some(href) = el.get_attribute("href") {
                if let Some(stem) = href.strip_suffix("_bg.wasm") {
                    if let (Some(glue), Some(wasm)) = (abs(&format!("{stem}.js")), abs(&href)) {
                        return Some((glue, wasm));
                    }
                }
            }
        }
    }

    // Fallback: any modulepreload `.js` → glue, derive wasm as `…_bg.wasm`.
    let links = doc.query_selector_all("link[rel=modulepreload]").ok()?;
    for i in 0..links.length() {
        let Some(node) = links.item(i) else { continue };
        let Ok(el) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        if let Some(href) = el.get_attribute("href") {
            let is_js = std::path::Path::new(&href)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("js"));
            if is_js {
                let wasm_href = href.strip_suffix(".js").map_or_else(
                    || format!("{href}_bg.wasm"),
                    |stem| format!("{stem}_bg.wasm"),
                );
                if let (Some(glue), Some(wasm)) = (abs(&href), abs(&wasm_href)) {
                    return Some((glue, wasm));
                }
            }
        }
    }
    None
}

/// Spawn the relay worker (same module, self-hosted boot) and return the bridge.
///
/// `Err` if the app glue can't be located (e.g. a non-Trunk bundler) or the boot
/// Blob can't be built. On success the returned [`ReactorBridge`] is a
/// `Stream<Item = WorkerOut> + Sink<RelayCommand>` the provider drives.
pub fn spawn_relay_bridge() -> Result<ReactorBridge<RelayReactor>, JsValue> {
    let (glue_abs, wasm_abs) = find_module_urls()
        .ok_or_else(|| JsValue::from_str("relay worker: app module URLs not found"))?;
    let boot_url = worker_boot_url(&glue_abs, &wasm_abs)?;

    let bridge = RelayReactor::spawner()
        .encoding::<JsonCodec>()
        // Use `boot_url` verbatim (our self-hosted Blob); don't let yew-agent
        // generate its own shim — that would run the app's `main` (the UI) in
        // the worker instead of the registrar.
        .with_loader(true)
        // The boot Blob is an ES module (`import` + top-level `await`).
        .as_module(true)
        .spawn(&boot_url);
    Ok(bridge)
}
