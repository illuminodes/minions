# worker_bench

An experiment comparing two ways to run the Nostr relay pool in a Yew app:

1. **In-thread** — the current `nostr-minions` pool. JSON parsing, dedup, and
   filter-matching all run on the UI thread.
2. **Web Worker** — a `yew-agent` **Reactor** (`RelayReactor`) that owns the
   WebSocket pool, `BoundedDedup`, and `note_matches_filter` on a *separate*
   thread. Matched notes are streamed back to the app over the reactor bridge.

The two panels subscribe to the same relays and the same kind-1 filter, so the
only difference is *where the work happens*. A main-thread "jank meter" samples
`requestAnimationFrame` deltas so you can watch UI-thread responsiveness while
notes stream in.

## The tradeoff being measured

The worker offloads JSON parse + dedup + filter off the UI thread, but pays a
**bincode serialize → postMessage → deserialize** cost for every `NostrNote`
that crosses the worker boundary. The in-thread pool passes notes by `Rc`-clone
within one thread (effectively free) but does all parsing on the UI thread.
This bench exists to see whether the worker is a net win under load.

## Layout

- `src/relay_worker.rs` — shared reactor + message types (`RelayCommand`, `RelayReactor`)
- `src/worker.rs` — Web Worker entry (`RelayReactor::registrar().register()`)
- `src/app.rs` — UI: both panels + jank meter
- `index.html` — Trunk wiring: `app` (main) + `worker` bins

## Run

```bash
trunk serve
```

Requires the `wasm32-unknown-unknown` target and [Trunk](https://trunkrs.dev/).
Built against yew 0.23 + yew-agent 0.5.

## Scope / limitations

- The worker path is **read-only ingestion**. Publishing (`send`) and the
  key/identity manager stay on the main thread, unchanged.
- The reactor reconnect logic is intentionally minimal (no exponential backoff
  yet) — the goal is to measure ingestion throughput, not match the in-thread
  pool's full lifecycle handling.
