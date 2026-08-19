#![warn(
    clippy::all,
    clippy::missing_errors_doc,
    clippy::style,
    clippy::unseparated_literal_suffix,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::future_not_send, clippy::missing_errors_doc)]

// Exactly one JSON backend, exactly one curve — mirrored from the nostro2
// stack so a misconfigured consumer fails here instead of deep in a
// dependency.
#[cfg(all(feature = "bourne", feature = "serde"))]
compile_error!("features `bourne` and `serde` are mutually exclusive; pick exactly one");
#[cfg(not(any(feature = "bourne", feature = "serde")))]
compile_error!("exactly one JSON backend feature must be enabled: `bourne` or `serde`");
#[cfg(all(feature = "k256", feature = "secp256k1"))]
compile_error!("features `k256` and `secp256k1` are mutually exclusive; pick exactly one");
#[cfg(not(any(feature = "k256", feature = "secp256k1")))]
compile_error!("exactly one curve backend feature must be enabled: `k256` or `secp256k1`");

// The pool's wire language and its ring arithmetic. Plain Rust with no browser
// bindings, so this tree also builds off `wasm32` — which is what lets
// `cargo test` run on the host. See `transport`'s module docs.
pub mod transport;

// The browser surface. Everything under here binds to `web-sys` / `yew` and so
// exists only on `wasm32`; off that target the crate is `transport` alone.
#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::*;

// Re-export nostro2 types directly for convenience.
// This allows: use minions::NostrNote instead of minions::nostro2::NostrNote
pub use nostro2::*;
pub use nostro2_signer::NostrKeypair;

// The NIP traits carry the signing/encryption methods `NostrKeypair` gets by
// blanket impl; consumers need them in scope to call those methods.
pub use nostro2_nips::{Nip17, Nip44, Nip46, Nip59};

// Make the full crates available for advanced usage.
// This allows: minions::nostro2_signer::... if needed
pub extern crate nostro2;
pub extern crate nostro2_nips;
pub extern crate nostro2_signer;
