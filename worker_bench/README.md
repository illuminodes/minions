# worker_bench

An experiment comparing four ways to run Nostr relay ingestion in a Yew app,
under a **controlled synthetic load** with quantitative instrumentation. At
live-relay rates all paths are trivially fast and look identical — the flood
mode is what makes the difference measurable.

1. **In-thread** — parse + dedup + filter run on the UI thread (what the
   `nostr-minions` pool does in its socket `onmessage`).
2. **Reactor** — a `yew-agent` **Reactor** (`RelayReactor`) does that work on
   a *separate* thread; matched notes stream back over the bridge (JSON encode +
   `postMessage`).
3. **Shared ring** — a worker sharing the wasm linear memory pushes
   `NostrNote`s into a `quetzalcoatl` SPSC ring. Zero serialization, zero
   per-note `postMessage`. **Needs nightly** (see below).
4. **SAB bytes** — a worker writes raw `["EVENT",…]` frames into a byte ring
   inside a `SharedArrayBuffer`, synchronized with `js_sys::Atomics`. Notes are
   serialized, but there is still no per-note `postMessage`. **Stable
   toolchain.**

Only **one path is mounted at a time** (toggle buttons), so they never compete.

## The tradeoff being measured

The worker offloads JSON parse + dedup + filter off the UI thread, but pays a
**postMessage → parse** cost for every note crossing the worker boundary. The in-thread path does all parsing on the UI thread but
passes notes by move within one thread (no boundary cost). This bench measures
whether moving the work off-thread is a net win under load.

> Note: yew-agent's default **Bincode** codec panics on `nostro2` types
> (`SequenceMustHaveLength`), so the bridge uses a custom **JSON** codec
> (`JsonCodec`). This also keeps the comparison honest — both paths do JSON
> (de)serialization; the worker just additionally crosses a thread boundary.

> The worker ships each matched note as the relay's ORIGINAL `["EVENT",…]`
> frame, not a re-encoded note. The worker already parsed it to dedup and match,
> so no second serialization is paid; the main thread parses once, exactly as
> the library's provider does.

### Why two shared-memory paths

Path 3 shares the wasm **linear memory**, so a `NostrNote`'s `String` pointers
stay valid across threads. That needs the whole binary — `std` included — built
with `+atomics`, which needs `-Z build-std`, which is nightly-only. A library
cannot ask its users for that.

Path 4 shares only a **byte buffer**. Each side keeps its own ordinary wasm
memory, so stable rustc is enough; `js_sys::Atomics` and `SharedArrayBuffer`
are stable APIs. The cost is serialization; the win that survives is the
absence of a structured clone and an event-loop task per note. Path 4 exists to
measure how much of path 3's advantage is reachable from a shippable
toolchain.

Both shared-memory paths need **cross-origin isolation** (COOP/COEP, see
`Trunk.toml`); the header bar reports `crossOriginIsolated`.

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
- `src/raf.rs` — `RafLoop`, a cancellable `requestAnimationFrame` chain. It
  calls `cancelAnimationFrame` before dropping the closure; dropping it alone
  leaves the browser holding a dead callback, which throws "closure invoked
  recursively or after being dropped" and aborts yew's effect queue.
- `src/drain.rs` — `Drain` + `use_drain`, the reactor path's rAF drain.
- `src/ring.rs` — path 3: shared-linear-memory SPSC ring of `NostrNote`.
- `src/sab.rs` — path 4: `SabRing`, the `SharedArrayBuffer` byte ring, plus the
  worker entry.
- `src/sab_panel.rs` — path 4's panel and its drain.
- `sab_index/` — the byte ring's pure index arithmetic, in a dependency-free
  crate so `cargo test` runs it **on the host**. The bench binary is wasm-only
  and cannot execute tests.
- `src/worker.rs` — Web Worker entry; calls `nostr_minions::relay_worker_main()`.

The reactor, its message types, `JsonCodec`, and the flood generator all live in
`nostr-minions` itself (`src/relay_pool/{worker,ingest,flood}.rs`). This bench
keeps NO copy of them: it depends on the library with the `bench-harness`
feature, so what it measures is exactly what applications run. Parse, dedup, and
filter-matching are all `NoteIngestor::ingest` — one implementation, shared by
the sockets and the flood.
- `src/app.rs` — UI: toggle, rate control, the panels, metrics panel, samplers.
- `index.html` — Trunk wiring: `app` (main) + `worker` bins.

## Test

```bash
cargo test --manifest-path sab_index/Cargo.toml --target x86_64-unknown-linux-gnu
```

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
