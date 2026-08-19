//! Giftwrap variants (NIP-59).
//!
//! `nostro2-nips` exposes one method per variant (`giftwrap`,
//! `replaceable_giftwrap`, `ephemeral_giftwrap`). This enum keeps the
//! variant a runtime value, which is what the `NostrId` store needs.

use nostro2_nips::Nip59;

/// Which NIP-59 giftwrap kind to build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GiftwrapScheme {
    /// Kind 1059, signed by a fresh throwaway key.
    Standard,
    /// Kind 10059, replaceable, signed by the sender.
    Replaceable,
    /// Kind 20059, ephemeral, signed by a fresh throwaway key.
    Ephemeral,
}

impl GiftwrapScheme {
    /// Wrap `rumor` for `peer_pubkey` with this scheme.
    ///
    /// # Errors
    /// Returns [`nostro2_nips::Nip59Error`] if sealing, encryption, or
    /// signing fails.
    pub fn wrap<S>(
        self,
        signer: &S,
        rumor: &mut nostro2::NostrNote,
        peer_pubkey: &str,
    ) -> Result<nostro2::NostrNote, nostro2_nips::Nip59Error>
    where
        S: Nip59 + Sized,
    {
        match self {
            Self::Standard => signer.giftwrap(rumor, peer_pubkey),
            Self::Replaceable => signer.replaceable_giftwrap(rumor, peer_pubkey),
            Self::Ephemeral => signer.ephemeral_giftwrap(rumor, peer_pubkey),
        }
    }
}
