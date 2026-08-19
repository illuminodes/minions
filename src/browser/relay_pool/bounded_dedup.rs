use std::collections::{HashSet, VecDeque};

/// Bounded deduplication tracker that prevents memory leaks
/// by keeping only the most recent N items
#[derive(Debug, Clone)]
pub struct BoundedDedup {
    ids: VecDeque<String>,
    set: HashSet<String>,
    max_size: usize,
}

impl BoundedDedup {
    /// Create a new bounded dedup tracker with a maximum size
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            ids: VecDeque::with_capacity(max_size),
            set: HashSet::with_capacity(max_size),
            max_size,
        }
    }

    /// Check if an ID has been seen recently
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.set.contains(id)
    }

    /// Insert a new ID, returning true if it was newly inserted
    /// Automatically removes oldest ID if capacity is exceeded
    pub fn insert(&mut self, id: String) -> bool {
        if self.set.contains(&id) {
            return false; // Already exists
        }

        self.ids.push_back(id.clone());
        self.set.insert(id);

        // Maintain bounded size by removing oldest
        if self.ids.len() > self.max_size {
            if let Some(old_id) = self.ids.pop_front() {
                self.set.remove(&old_id);
            }
        }

        true
    }

    /// Get current number of tracked IDs
    #[must_use]
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Check if tracker is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Clear all tracked IDs
    pub fn clear(&mut self) {
        self.ids.clear();
        self.set.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_dedup() {
        let mut dedup = BoundedDedup::new(3);

        // Insert first 3
        assert!(dedup.insert("id1".to_string()));
        assert!(dedup.insert("id2".to_string()));
        assert!(dedup.insert("id3".to_string()));
        assert_eq!(dedup.len(), 3);

        // Duplicate should return false
        assert!(!dedup.insert("id1".to_string()));
        assert_eq!(dedup.len(), 3);

        // Insert 4th should evict oldest
        assert!(dedup.insert("id4".to_string()));
        assert_eq!(dedup.len(), 3);
        assert!(!dedup.contains("id1")); // Evicted
        assert!(dedup.contains("id2"));
        assert!(dedup.contains("id3"));
        assert!(dedup.contains("id4"));
    }
}
