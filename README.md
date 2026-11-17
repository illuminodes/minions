# Minions

A framework for building performant and secure PWAs in (mostly) full-stack Rust. 

We aim to provide simple building blocks for apps that are fast, secure, and easy to build.
Open source widget libraries are integrated to Nostr data feeds to provide interactivity and 
real-time data updates.

## Build and run

The app uses Yew to build a WASM client side application. Styling is handled using TailwindCSS.


To build and run the project, you should have installed:

- [Rust](https://www.rust-lang.org/)
- [TailwindCSS](https://tailwindcss.com/)
- [Trunk](https://trunkrs.dev/)

To build a new application, clone the repository and run the following command:

```bash
trunk serve
```

This will build the project and start a development server.

## Cargo features

The nostro2 stack requires one curve backend and one JSON backend, and each
choice is exclusive. `nostr-minions` exposes both choices as its own features:

| Feature | Choice | Default |
| --- | --- | --- |
| `k256` | pure-Rust curve | yes |
| `secp256k1` | libsecp256k1 (C) curve | |
| `serde` | `serde_json` for Nostr wire types | yes |
| `bourne` | `json-bourne` for Nostr wire types | |
| `sab-transport` | `SharedArrayBuffer` note path; needs a cross-origin isolated page | |
| `bench-harness` | synthetic flood on the relay reactor, for `worker_bench` | |
| `test-components` | test-only UI components | |

To use a different backend, turn off the defaults and pick one feature from each pair:

```toml
nostr-minions = { version = "0.5", default-features = false, features = ["secp256k1", "bourne"] }
```

The build fails with a `compile_error!` if both features of a pair are enabled, or neither.
If your app also depends on `nostro2`, `nostro2-nips` or `nostro2-signer`
directly, declare them with `default-features = false`. Otherwise their
defaults also turn on `k256` and `serde`, and the build fails with a backend conflict.

To encode or parse Nostr types under either backend, use `nostr_minions::NostrJson`
instead of calling `serde_json` directly.

The `secp256k1` backend on `wasm32-unknown-unknown` needs
`CFLAGS_wasm32_unknown_unknown=-Wno-implicit-function-declaration` in the
build environment.

## Key storage

`use_create_local_key` saves the user's secret key in IndexedDB (database
`nostr_db`, store `user_identity`). The key is held as a Web Crypto
`CryptoKey`, but it is imported as **extractable**: signing and encryption run
in wasm, so the raw bytes have to be readable.

The browser therefore does not protect this key from code running on your
origin. Any script on the page, such as an injected script or a compromised
dependency, can read the record and export the secret. Treat it like a key
kept in `localStorage`:

- serve the app with a strict Content-Security-Policy and avoid third-party
  scripts;
- if the key must stay out of the page, sign with a NIP-07 browser extension
  or a NIP-46 remote signer instead of a locally stored key. The key hooks
  in this crate manage a local key only, so that wiring is up to the app.

### "Warm" Reloading

The build script will recognize any HTML content inside Yew's functional components and reload new changes on save.
Due to Rust's compile times, this will not be instant.

### Contributing

Bounties to help drive open contribution forward will be posted as issues on the repository.
All rewards are paid in Bitcoin.

