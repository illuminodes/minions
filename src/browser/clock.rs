//! Wall-clock time for the browser.

/// The browser wall clock, in Nostr's `created_at` unit (Unix seconds).
pub struct WallClock;

impl WallClock {
    /// Return the current Unix time in seconds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn unix_seconds() -> i64 {
        (web_sys::js_sys::Date::now() / 1000.0) as i64
    }
}
