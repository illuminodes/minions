use nostro2::{NostrNote, NostrSubscription};
use yew::Callback;

/// Unique identifier for a subscription
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId(String);

impl SubscriptionId {
    /// Create a new unique subscription ID
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
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

/// Information about an active subscription
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

/// Check if a note matches a subscription filter
#[must_use]
pub fn note_matches_filter(note: &NostrNote, filter: &NostrSubscription) -> bool {
    // Check kinds
    if let Some(kinds) = &filter.kinds {
        if !kinds.contains(&note.kind) {
            return false;
        }
    }

    // Check authors
    if let Some(authors) = &filter.authors {
        if !authors.iter().any(|a| a == &note.pubkey) {
            return false;
        }
    }

    // Check IDs
    if let Some(ids) = &filter.ids {
        if let Some(note_id) = &note.id {
            if !ids.iter().any(|id| id == note_id) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check since/until timestamps
    if let Some(since) = filter.since {
        #[allow(clippy::cast_possible_wrap)]
        if note.created_at < since as i64 {
            return false;
        }
    }

    if let Some(until) = filter.until {
        #[allow(clippy::cast_possible_wrap)]
        if note.created_at > until as i64 {
            return false;
        }
    }

    // Check tag filters (e.g. "#p", "#e")
    if let Some(ref tag_filters) = filter.tags {
        for (key, values) in tag_filters {
            let letter = key.strip_prefix('#').unwrap_or(key);
            let note_vals: Vec<&str> = note.tags.find_tags_ref(letter);
            if !values.iter().any(|v| note_vals.contains(&v.as_str())) {
                return false;
            }
        }
    }

    true
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

    #[test]
    fn test_note_matches_filter_by_kind() {
        let note = NostrNote {
            kind: 1,
            pubkey: "abc123".to_string(),
            created_at: 1000,
            ..Default::default()
        };

        let filter = NostrSubscription {
            kinds: Some(vec![1, 2]),
            ..Default::default()
        };

        assert!(note_matches_filter(&note, &filter));

        let filter_no_match = NostrSubscription {
            kinds: Some(vec![3, 4]),
            ..Default::default()
        };

        assert!(!note_matches_filter(&note, &filter_no_match));
    }

    #[test]
    fn test_note_matches_filter_by_author() {
        let note = NostrNote {
            kind: 1,
            pubkey: "alice".to_string(),
            created_at: 1000,
            ..Default::default()
        };

        let filter = NostrSubscription {
            authors: Some(vec!["alice".to_string(), "bob".to_string()]),
            ..Default::default()
        };

        assert!(note_matches_filter(&note, &filter));

        let filter_no_match = NostrSubscription {
            authors: Some(vec!["bob".to_string()]),
            ..Default::default()
        };

        assert!(!note_matches_filter(&note, &filter_no_match));
    }

    #[test]
    fn test_note_matches_filter_by_timestamp() {
        let note = NostrNote {
            kind: 1,
            pubkey: "alice".to_string(),
            created_at: 1000,
            ..Default::default()
        };

        let filter_since = NostrSubscription {
            since: Some(900),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter_since));

        let filter_since_fail = NostrSubscription {
            since: Some(1100),
            ..Default::default()
        };
        assert!(!note_matches_filter(&note, &filter_since_fail));

        let filter_until = NostrSubscription {
            until: Some(1100),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter_until));

        let filter_until_fail = NostrSubscription {
            until: Some(900),
            ..Default::default()
        };
        assert!(!note_matches_filter(&note, &filter_until_fail));
    }

    fn note_with_tags(tags: Vec<Vec<String>>) -> NostrNote {
        NostrNote {
            kind: 1,
            pubkey: "alice".to_string(),
            created_at: 1000,
            tags: tags.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_note_matches_filter_by_tag() {
        let note = note_with_tags(vec![
            vec!["p".to_string(), "bob_pk".to_string()],
            vec!["e".to_string(), "event123".to_string()],
        ]);

        let mut tags = std::collections::HashMap::new();
        tags.insert("#p".to_string(), vec!["bob_pk".to_string()]);

        let filter = NostrSubscription {
            tags: Some(tags),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter));
    }

    #[test]
    fn test_note_does_not_match_missing_tag() {
        let note = note_with_tags(vec![vec!["p".to_string(), "bob_pk".to_string()]]);

        let mut tags = std::collections::HashMap::new();
        tags.insert("#p".to_string(), vec!["charlie_pk".to_string()]);

        let filter = NostrSubscription {
            tags: Some(tags),
            ..Default::default()
        };
        assert!(!note_matches_filter(&note, &filter));
    }

    #[test]
    fn test_note_matches_multiple_tag_filters_and() {
        let note = note_with_tags(vec![
            vec!["p".to_string(), "bob_pk".to_string()],
            vec!["e".to_string(), "event123".to_string()],
        ]);

        let mut tags = std::collections::HashMap::new();
        tags.insert("#p".to_string(), vec!["bob_pk".to_string()]);
        tags.insert("#e".to_string(), vec!["event123".to_string()]);

        let filter = NostrSubscription {
            tags: Some(tags),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter));

        // Fails if one tag filter doesn't match
        let mut tags_partial = std::collections::HashMap::new();
        tags_partial.insert("#p".to_string(), vec!["bob_pk".to_string()]);
        tags_partial.insert("#e".to_string(), vec!["other_event".to_string()]);

        let filter_partial = NostrSubscription {
            tags: Some(tags_partial),
            ..Default::default()
        };
        assert!(!note_matches_filter(&note, &filter_partial));
    }

    #[test]
    fn test_tag_filter_strips_hash_prefix() {
        let note = note_with_tags(vec![vec!["p".to_string(), "bob_pk".to_string()]]);

        // With # prefix
        let mut tags_with_hash = std::collections::HashMap::new();
        tags_with_hash.insert("#p".to_string(), vec!["bob_pk".to_string()]);
        let filter = NostrSubscription {
            tags: Some(tags_with_hash),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter));

        // Without # prefix (should also work)
        let mut tags_without_hash = std::collections::HashMap::new();
        tags_without_hash.insert("p".to_string(), vec!["bob_pk".to_string()]);
        let filter2 = NostrSubscription {
            tags: Some(tags_without_hash),
            ..Default::default()
        };
        assert!(note_matches_filter(&note, &filter2));
    }
}
