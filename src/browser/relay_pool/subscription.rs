use nostro2::{NostrNote, NostrSubscription};
use yew::Callback;

/// Unique identifier for a subscription
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId(String);

impl SubscriptionId {
    /// Create a new unique subscription ID
    ///
    /// # Panics
    /// Panics if the browser global `window`, the Web Crypto API, or
    /// `getRandomValues` is unavailable — none of which occur in a normal
    /// browser context.
    #[must_use]
    pub fn new() -> Self {
        use std::fmt::Write as _;
        let mut buf = [0_u8; 16];
        web_sys::window()
            .expect("no global window")
            .crypto()
            .expect("no crypto on window")
            .get_random_values_with_u8_array(&mut buf)
            .expect("get_random_values failed");
        let hex = buf.iter().fold(String::with_capacity(32), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        });
        Self(hex)
    }

    /// Create a subscription ID from an existing string
    #[must_use]
    pub const fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the subscription ID as a string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SubscriptionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about an active note subscription
#[derive(Clone)]
pub struct SubscriptionInfo {
    pub id: SubscriptionId,
    pub filter: NostrSubscription,
    pub callback: Callback<NostrNote>,
    pub created_at: i64,
    pub note_count: usize,
}

impl std::fmt::Debug for SubscriptionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionInfo")
            .field("id", &self.id)
            .field("filter", &self.filter)
            .field("created_at", &self.created_at)
            .field("note_count", &self.note_count)
            .finish_non_exhaustive()
    }
}

/// Information about a relay event subscription
#[derive(Clone)]
pub struct RelayEventSubscription {
    pub id: SubscriptionId,
    pub callback: Callback<nostro2::NostrRelayEvent>,
}

impl std::fmt::Debug for RelayEventSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayEventSubscription")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_id_uniqueness() {
        let id1 = SubscriptionId::new();
        let id2 = SubscriptionId::new();
        assert_ne!(id1, id2);
    }
}
