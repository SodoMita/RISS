//! A tiny insertion-ordered LRU cache.
//!
//! Application icons are the only unbounded thing a launcher renders: a user
//! with 400 applications and an "all apps" visit would pin 400 decoded RGBA
//! textures forever. KISS bounds its icon handling the same way — icons are
//! decoded once, memoized on the `Result` object and written to a PNG cache on
//! disk, and old cache files are pruned (`IconsHandler.clearOldCache`).
//!
//! Lookups are O(1) and take `&str`, so a result row does not have to allocate
//! a key to ask whether its icon is already on the GPU. Eviction happens on
//! insertion only, by scanning for the oldest stamp, which keeps the cache
//! dependency-free (no `lru` crate, so `Cargo.lock` stays untouched).

use std::collections::HashMap;

pub(crate) struct LruCache<V> {
    capacity: usize,
    /// Monotonic clock: bumped on every hit, stamped on every insertion.
    clock: u64,
    map: HashMap<String, (u64, V)>,
}

impl<V> LruCache<V> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            clock: 0,
            map: HashMap::new(),
        }
    }

    /// Look `key` up and mark it as recently used.
    pub(crate) fn get(&mut self, key: &str) -> Option<&V> {
        self.map.get_mut(key).map(|entry| {
            self.clock += 1;
            entry.0 = self.clock;
            &entry.1
        })
    }

    /// Insert `value`, evicting the least recently used entry first if the
    /// cache is full. Returns the evicted key, if any.
    pub(crate) fn insert(&mut self, key: &str, value: V) -> Option<String> {
        let mut evicted = None;
        if self.map.len() >= self.capacity && !self.map.contains_key(key) {
            if let Some(oldest) = self
                .map
                .iter()
                .min_by_key(|(_, (stamp, _))| *stamp)
                .map(|(key, _)| key.clone())
            {
                self.map.remove(&oldest);
                evicted = Some(oldest);
            }
        }
        self.clock += 1;
        self.map.insert(key.to_owned(), (self.clock, value));
        evicted
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.clock = 0;
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_refreshes_recency() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get("a"), Some(&1));
        // Inserting "c" evicts "b", the least recently used, not "a".
        cache.insert("c", 3);
        assert_eq!(cache.get("b"), None);
        assert_eq!(cache.get("a"), Some(&1));
        assert_eq!(cache.get("c"), Some(&3));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn reinserting_does_not_evict() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("a", 10);
        assert_eq!(cache.get("a"), Some(&10));
        assert_eq!(cache.get("b"), Some(&2));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn capacity_is_never_zero() {
        let mut cache = LruCache::new(0);
        cache.insert("a", 1);
        assert_eq!(cache.get("a"), Some(&1));
        cache.insert("b", 2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn clear_forgets_everything() {
        let mut cache = LruCache::new(4);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.get("a"), None);
    }
}
