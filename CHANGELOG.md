# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0]

### Added

- README section on choosing the curve (`k256` / `secp256k1`) and JSON
  (`serde` / `bourne`) backends.
- `NostrId::decrypt_legacy_note` decrypts legacy `0x31` NIP-44 content after
  verifying the event's id and signature, so data encrypted before 0.5.0 can
  be read and re-encrypted with `sign_encrypted_note`.
- `MinionError::UnverifiedEvent`, returned when that check fails. Code that
  matches `MinionError` exhaustively needs a new arm.

### Changed

- **Breaking.** Nostr stack moved to `nostro2` 0.9, `nostro2-nips` 0.7 and
  `nostro2-signer` 0.8. These types are re-exported from this crate, so
  consumers that name them directly must move to the same versions.
- **Breaking, `bourne` backend only.** `json-bourne` 0.2 -> 0.3. Its `Error`
  type is part of `nostro2`'s public API.
- **Behaviour.** `NostrId::decrypt_note` no longer decrypts legacy
  `0x31` NIP-44 payloads, because `nostro2-nips` 0.6 removed that path from
  `nip_44_decrypt`; it returns `NIP-44 Error: unsupported NIP-44 version
  byte: 0x31` for them. Use `decrypt_legacy_note` to read such data. NIP-17 rumors now carry a `p` tag for the recipient.
  Both changes come from the upstream security release; no signature here
  changed.

## [0.4.0]

### Added

- `transport` module: the wire vocabulary and ring arithmetic, as plain Rust
  with no browser bindings — `Envelope`, `Inbound`, `Outbound`, `NoteCodec`,
  `RingIndex`, `FrameQueue`, `FrameSink`.
- `sab-transport` feature: the `SharedArrayBuffer` note path. The worker writes
  matched notes into a shared byte ring as a flat binary encoding instead of
  posting one JSON message per note; the UI thread drains the ring on an
  animation frame. Measured against the JSON bridge at 3000 notes/s: 0.0%
  dropped frames versus 11%, worst frame 17ms versus 83ms.

  Off by default, and enabling it is not sufficient on its own. A
  `SharedArrayBuffer` exists only on a cross-origin isolated page, which needs
  `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp` response headers that a library
  cannot set for its consumer. The pool probes `crossOriginIsolated` at runtime
  and falls back to the JSON bridge when the page is not isolated.
- The relay pool runs as a `yew-agent` Reactor in a self-spawned Web Worker.
  WebSockets, deduplication, and filter matching run off the UI thread.
- `bench-harness` feature: exposes the synthetic flood generator on the relay
  reactor so `worker_bench` drives the real ingestion path.

### Changed

- **Breaking.** Browser-facing modules moved under `browser`. Update imports:

  | Before                     | After                               |
  | -------------------------- | ----------------------------------- |
  | `nostr_minions::relay_pool` | `nostr_minions::browser::relay_pool` |
  | `nostr_minions::key_manager` | `nostr_minions::browser::key_manager` |
  | `nostr_minions::idb_manager` | `nostr_minions::browser::idb_manager` |
  | `nostr_minions::crypto`     | `nostr_minions::browser::crypto`     |
  | `nostr_minions::clock`      | `nostr_minions::browser::clock`      |
  | `nostr_minions::nostr_json` | `nostr_minions::browser::nostr_json` |

  The `browser` tree is gated to `wasm32` and so are its dependencies. Off that
  target the crate is `transport` alone, which is why `cargo test` now builds
  and runs on the host.
- Bumped `yew` 0.21 to 0.23.
- The nostro2 curve and JSON backends are selectable through this crate's own
  `k256` / `secp256k1` and `serde` / `bourne` features. The default is
  `k256` + `serde`.

### Fixed

- Context updates never reached consumers. `notify_change` clones the store, but
  every field is an `Rc`, so the clone shares the same `Cell`; `PartialEq`
  compared a cell against itself and always reported equality. `NostrRelayPool`
  now carries a `version` counter that `PartialEq` compares, so transport-status
  and relay-health consumers re-render.
- Transport negotiation could deadlock when both peers waited on an offer.
- WebSocket lifecycle, reconnection bounds, a send race, and render storms.
- Subscription IDs now match between hooks and the relay `REQ` messages.
- Cross-thread latency uses a wall clock, so worker and UI timestamps compare.
- `NoteCodec` distinguishes an absent field from a malformed frame through a
  dedicated type rather than a nested `Option`.

### Removed

- The `wire_envelope`, `note_codec`, `sab_index`, and `frame_queue` path crates.
  Their contents are the `transport` module, so `nostr-minions` publishes as one
  self-contained crate.
- The `uuid` and `thiserror` dependencies.

## [0.3.1]

Refer to the git history for releases before this changelog.
