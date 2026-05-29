# worker_bench

An experiment comparing two ways to run Nostr relay ingestion in a Yew app,
under a **controlled synthetic load** with quantitative instrumentation. At
live-relay rates both paths are trivially fast and look identical — the flood
mode is what makes the difference measurable.

1. **In-thread** — parse + dedup + filter run on the UI thread (what the
   `nostr-minions` pool does in its socket `onmessage`).
2. **Web Worker** — a `yew-agent` **Reactor** (`RelayReactor`) does that work on
   a *separate* thread; matched notes stream back over the bridge (JSON encode +
   `postMessage`).

Only **one path is mounted at a time** (toggle buttons), so they never compete.

## The tradeoff being measured

The worker offloads JSON parse + dedup + filter off the UI thread, but pays a
**serialize → postMessage → deserialize** cost for every `NostrNote` crossing
the worker boundary. The in-thread path does all parsing on the UI thread but
passes notes by move within one thread (no boundary cost). This bench measures
whether moving the work off-thread is a net win under load.

> Note: yew-agent's default **Bincode** codec panics on `nostro2` types
> (`SequenceMustHaveLength`), so the bridge uses a custom **JSON** codec
> (`JsonCodec`). This also keeps the comparison honest — both paths do JSON
> (de)serialization; the worker just additionally crosses a thread boundary.

## Synthetic flood

Each panel has a **Flood Ns** button and a shared rate control (notes/sec). The
flood generates raw NIP-01 `["EVENT",…]` JSON strings and runs them through the
*same* `parse → dedup → filter` path each architecture really uses — on the main
thread for in-thread, inside the worker for the worker path. Each note carries
its emit timestamp in `content` (`BENCH:<ms>:…`) so end-to-end latency survives
the round-trip.

## Metrics (live panel + once-per-second `[BENCH]` console dump)

- **Main-thread jank histogram** — `requestAnimationFrame` interval buckets
  (`<17ms / 17–50 / 50–100 / 100ms+`), worst frame, and jank %. The headline:
  how much the UI thread stalls. The worker should keep this near-zero; the
  in-thread path should spike under load.
- **Throughput** — notes produced vs rendered per second, and **backlog**
  (produced − rendered; turns red when the path can't keep up).
- **Per-note latency** — p50 / p95 / p99 end-to-end (arrival → render).

Open DevTools → Console to copy the structured `[BENCH]` snapshots; the
histogram also surfaces UI-thread stalls you can corroborate in the Performance
panel.

## Layout

- `src/metrics.rs` — instrumentation: jank histogram, latency tracker,
  throughput counter, synthetic note generator, `performance.now()` clock.
  Shared by both bins.
- `src/relay_worker.rs` — reactor + message types (`RelayCommand`,
  `RelayReactor`), `JsonCodec`, and the in-worker flood generator.
- `src/worker.rs` — Web Worker entry (`registrar().encoding::<JsonCodec>().register()`).
- `src/app.rs` — UI: toggle, rate control, both panels, metrics panel, samplers.
- `index.html` — Trunk wiring: `app` (main) + `worker` bins.

## Run

```bash
trunk serve
```

Pick a path, set a rate (try 2000), click **Flood 5s**, and watch the metrics
panel + console. Then switch paths and repeat at the same rate to compare.

Requires the `wasm32-unknown-unknown` target and [Trunk](https://trunkrs.dev/).
Built against yew 0.23 + yew-agent 0.5.

## Scope / limitations

- The worker path is **read-only ingestion**. Publishing and the key/identity
  manager stay on the main thread, unchanged.
- Live-relay streaming was dropped in favor of the synthetic flood — it's the
  only reliable way to make the difference visible and reproducible. (Real
  relays connect fine; they're just too slow/bursty to benchmark with.)
- The flood emits in ~16ms ticks so notes spread over wall-clock time rather
  than one blocking burst, matching a real high-rate feed.
