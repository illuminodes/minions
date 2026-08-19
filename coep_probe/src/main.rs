//! Reports whether a URL still loads on a cross-origin isolated page.
//!
//! Enabling `SharedArrayBuffer` means sending `Cross-Origin-Embedder-Policy:
//! require-corp`, which blocks cross-origin subresources that do not opt in.
//! This tool asks each URL's server directly, so the decision rests on the
//! headers the server actually sends.
//!
//! ```text
//! cargo run --manifest-path coep_probe/Cargo.toml -- <url>...
//! cargo run --manifest-path coep_probe/Cargo.toml -- --frame <url>...
//! ```
//!
//! With no arguments it checks the CDN scripts in `test_app/index.html`.
//!
//! `--frame` judges the URLs as `<iframe>` documents instead of subresources.
//! The distinction is not cosmetic: a framed document must send COEP ITSELF,
//! and the `Cross-Origin-Resource-Policy` header that lets a script through
//! says nothing about a frame.

#![warn(clippy::pedantic, clippy::nursery)]

mod probe;
mod verdict;

/// The command-line entry point.
struct Cli;

impl Cli {
    /// The URLs `test_app/index.html` loads, checked when none are given.
    const DEFAULT_URLS: [&'static str; 2] = [
        "https://cdn.jsdelivr.net/npm/fullcalendar@6.1.19/index.global.min.js",
        "https://cdn.jsdelivr.net/npm/@fullcalendar/core@6.1.19/locales/es.global.min.js",
    ];

    fn run() -> std::process::ExitCode {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let usage = if args.iter().any(|a| a == "--frame") {
            probe::Usage::Frame
        } else {
            probe::Usage::Subresource
        };
        let given: Vec<&str> = args
            .iter()
            .map(String::as_str)
            .filter(|a| !a.starts_with("--"))
            .collect();
        let urls: Vec<&str> = if given.is_empty() {
            Self::DEFAULT_URLS.to_vec()
        } else {
            given
        };

        let probe = probe::Probe::new();
        let mut all_load = true;

        let kind = match usage {
            probe::Usage::Subresource => "subresource",
            probe::Usage::Frame => "iframe document",
        };
        println!(
            "Checking {} URL(s) as {kind}, against COEP: require-corp\n",
            urls.len()
        );
        for url in urls {
            match probe.check(url, usage) {
                Ok(probed) => {
                    if !probed.found() || !probed.loads() {
                        all_load = false;
                    }
                    println!("{}\n", probed.report());
                }
                Err(e) => {
                    all_load = false;
                    println!("[!] {e}\n");
                }
            }
        }

        if all_load {
            println!("All URLs load on an isolated page.");
            std::process::ExitCode::SUCCESS
        } else {
            println!("Some URLs would break. Self-host them, or do not isolate.");
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() -> std::process::ExitCode {
    Cli::run()
}
