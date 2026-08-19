//! Fetches one URL and reads the two headers COEP cares about.

use crate::verdict::{FrameVerdict, Verdict};

/// How the page intends to load a URL.
///
/// The two are judged by different rules, so the caller must say which it
/// means. Guessing from the file extension would be wrong exactly where it
/// matters most.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Usage {
    /// A `<script>`, `<link>`, `<img>`, or font.
    Subresource,
    /// A document inside an `<iframe>`.
    Frame,
}

/// The result of probing one URL.
pub struct Probed {
    url: String,
    status: u16,
    outcome: Outcome,
}

/// The verdict, in whichever of the two vocabularies applies.
enum Outcome {
    Subresource(Verdict),
    Frame(FrameVerdict),
}

impl Outcome {
    const fn loads(&self) -> bool {
        match self {
            Self::Subresource(verdict) => verdict.loads(),
            Self::Frame(verdict) => verdict.loads(),
        }
    }

    fn advice(&self) -> String {
        match self {
            Self::Subresource(verdict) => verdict.advice(),
            Self::Frame(verdict) => verdict.advice(),
        }
    }
}

impl Probed {
    /// Whether the URL loads under COEP, as the requested usage.
    #[must_use]
    pub const fn loads(&self) -> bool {
        self.outcome.loads()
    }

    /// Whether the URL itself resolved.
    ///
    /// Reported separately from the verdict because a 404 is a different
    /// problem from a COEP block, and a broken path can hide behind a passing
    /// header check.
    #[must_use]
    pub const fn found(&self) -> bool {
        self.status < 400
    }

    /// One line for a terminal.
    #[must_use]
    pub fn report(&self) -> String {
        let mark = if !self.found() {
            "!"
        } else if self.outcome.loads() {
            "+"
        } else {
            "x"
        };
        let detail = if self.found() {
            self.outcome.advice()
        } else {
            format!("HTTP {} — the URL itself is wrong", self.status)
        };
        format!("[{mark}] {}\n    {detail}", self.url)
    }
}

/// Asks a server what it says about one URL.
pub struct Probe {
    agent: ureq::Agent,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe {
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(15))
                .build(),
        }
    }

    /// Probe one URL.
    ///
    /// The request carries an `Origin` header because that is what a browser
    /// sends for a `crossorigin` tag, and some CDNs answer with
    /// `Access-Control-Allow-Origin` only when asked that way.
    ///
    /// # Errors
    ///
    /// Returns the transport error when the host cannot be reached at all.
    pub fn check(&self, url: &str, usage: Usage) -> Result<Probed, String> {
        let response = self
            .agent
            .get(url)
            .set("Origin", "https://localhost")
            .call();

        let response = match response {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(e) => return Err(format!("{url}: {e}")),
        };

        let outcome = match usage {
            Usage::Subresource => Outcome::Subresource(Verdict::of(
                response.header("Cross-Origin-Resource-Policy"),
                response.header("Access-Control-Allow-Origin"),
            )),
            Usage::Frame => Outcome::Frame(FrameVerdict::of(
                response.header("Cross-Origin-Embedder-Policy"),
            )),
        };

        Ok(Probed {
            url: url.to_string(),
            status: response.status(),
            outcome,
        })
    }
}
