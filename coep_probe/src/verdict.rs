//! The COEP rule, applied to response headers.
//!
//! This is separate from the fetching so it can be tested without a network:
//! the rule is the part that must be right, and the rule is pure.
//!
//! # The rule
//!
//! `Cross-Origin-Embedder-Policy: require-corp` lets a cross-origin subresource
//! load only when the response OPTS IN, in one of two ways:
//!
//! - `Cross-Origin-Resource-Policy: cross-origin` (or `same-site` for a
//!   same-site URL), which opts in with no CORS needed.
//! - A successful CORS response, which needs `Access-Control-Allow-Origin` AND
//!   a `crossorigin` attribute on the tag, so the browser sends the request in
//!   CORS mode.
//!
//! Without either, the browser gets an opaque response and COEP blocks it.

/// What a response's headers mean for a cross-origin isolated page.
///
/// These answers apply to SUBRESOURCES: scripts, styles, images, fonts. They do
/// NOT apply to a document in an `<iframe>`, which is governed by
/// [`FrameVerdict`] instead.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// `Cross-Origin-Resource-Policy` opts in on its own.
    AllowedByCorp { value: String },
    /// CORS opts in, but only if the tag carries `crossorigin`.
    AllowedByCorsWithAttribute { allow_origin: String },
    /// COEP blocks this. Nothing on the embedding page can change it.
    Blocked,
}

impl Verdict {
    /// Apply the rule to the two headers that decide it.
    ///
    /// CORP is checked first because it needs no attribute on the tag, so it is
    /// the stronger of the two answers.
    #[must_use]
    pub fn of(corp: Option<&str>, allow_origin: Option<&str>) -> Self {
        if let Some(value) = corp.map(str::trim).filter(|v| !v.is_empty()) {
            let value = value.to_ascii_lowercase();
            if value == "cross-origin" || value == "same-site" {
                return Self::AllowedByCorp { value };
            }
        }
        if let Some(origin) = allow_origin.map(str::trim).filter(|v| !v.is_empty()) {
            return Self::AllowedByCorsWithAttribute {
                allow_origin: origin.to_string(),
            };
        }
        Self::Blocked
    }

    /// Whether the resource loads at all on an isolated page.
    #[must_use]
    pub const fn loads(&self) -> bool {
        !matches!(self, Self::Blocked)
    }

    /// What to do about it, in one line.
    #[must_use]
    pub fn advice(&self) -> String {
        match self {
            Self::AllowedByCorp { value } => format!(
                "loads as-is, with or without crossorigin \
                 (Cross-Origin-Resource-Policy: {value})"
            ),
            Self::AllowedByCorsWithAttribute { allow_origin } => format!(
                "loads ONLY with crossorigin=\"anonymous\" on the tag \
                 (Access-Control-Allow-Origin: {allow_origin})"
            ),
            Self::Blocked => {
                "BLOCKED under COEP: no CORP header and no CORS. Self-host it, \
                 or do not isolate the page."
                    .to_string()
            }
        }
    }
}

/// What a response's headers mean for a document loaded in an `<iframe>`.
///
/// A framed document is held to a different rule than a subresource, and this
/// is the trap the tool exists to prevent: a CDN-style
/// `Cross-Origin-Resource-Policy: cross-origin` header says NOTHING about
/// whether a frame is allowed. The frame must send
/// `Cross-Origin-Embedder-Policy` ITSELF.
///
/// `YouTube`, Stripe, and most ad frames send CORP and no COEP, so reading CORP
/// here would report a pass for an embed that a browser refuses to display.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FrameVerdict {
    /// The framed document sends COEP, so it may be embedded.
    Allowed { coep: String },
    /// The framed document does not send COEP. Nothing on the embedding page
    /// can fix this; only the third party can.
    Blocked,
}

impl FrameVerdict {
    /// Apply the frame rule to the one header that decides it.
    #[must_use]
    pub fn of(coep: Option<&str>) -> Self {
        let Some(value) = coep.map(str::trim).filter(|v| !v.is_empty()) else {
            return Self::Blocked;
        };
        let value = value.to_ascii_lowercase();
        if value.starts_with("require-corp") || value.starts_with("credentialless") {
            return Self::Allowed { coep: value };
        }
        Self::Blocked
    }

    /// Whether the frame can be embedded on an isolated page.
    #[must_use]
    pub const fn loads(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    /// What to do about it, in one line.
    #[must_use]
    pub fn advice(&self) -> String {
        match self {
            Self::Allowed { coep } => {
                format!("embeddable in an iframe (Cross-Origin-Embedder-Policy: {coep})")
            }
            Self::Blocked => "NOT embeddable in an iframe on an isolated page: the \
                 framed document sends no COEP. Only the third party can fix \
                 this. Do not isolate if you need this embed."
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameVerdict, Verdict};

    #[test]
    fn corp_cross_origin_needs_no_attribute() {
        let verdict = Verdict::of(Some("cross-origin"), None);
        assert!(verdict.loads());
        assert!(verdict.advice().contains("with or without"));
    }

    #[test]
    fn corp_same_site_also_opts_in() {
        assert!(Verdict::of(Some("same-site"), None).loads());
    }

    #[test]
    fn corp_same_origin_does_not_opt_in_for_a_third_party() {
        assert_eq!(Verdict::of(Some("same-origin"), None), Verdict::Blocked);
    }

    #[test]
    fn cors_alone_demands_the_attribute() {
        let verdict = Verdict::of(None, Some("*"));
        assert!(verdict.loads());
        assert!(verdict.advice().contains("ONLY with crossorigin"));
    }

    #[test]
    fn no_headers_is_blocked() {
        let verdict = Verdict::of(None, None);
        assert!(!verdict.loads());
        assert_eq!(verdict, Verdict::Blocked);
    }

    #[test]
    fn corp_wins_over_cors_because_it_needs_no_attribute() {
        let verdict = Verdict::of(Some("cross-origin"), Some("*"));
        assert!(matches!(verdict, Verdict::AllowedByCorp { .. }));
    }

    #[test]
    fn header_case_and_padding_do_not_change_the_answer() {
        assert!(Verdict::of(Some("  Cross-Origin  "), None).loads());
    }

    #[test]
    fn an_empty_header_counts_as_absent() {
        assert_eq!(Verdict::of(Some("   "), Some("")), Verdict::Blocked);
    }

    #[test]
    fn a_frame_needs_coep_not_corp() {
        assert_eq!(FrameVerdict::of(None), FrameVerdict::Blocked);
        assert!(FrameVerdict::of(Some("require-corp")).loads());
    }

    #[test]
    fn credentialless_also_lets_a_frame_embed() {
        assert!(FrameVerdict::of(Some("credentialless")).loads());
    }

    #[test]
    fn unsafe_none_does_not_let_a_frame_embed() {
        assert_eq!(
            FrameVerdict::of(Some("unsafe-none")),
            FrameVerdict::Blocked
        );
    }

    #[test]
    fn a_frame_verdict_ignores_the_corp_header_entirely() {
        // The exact trap: YouTube sends CORP: cross-origin and no COEP.
        // As a subresource that passes; as a frame it must not.
        assert!(Verdict::of(Some("cross-origin"), None).loads());
        assert!(!FrameVerdict::of(None).loads());
    }
}
