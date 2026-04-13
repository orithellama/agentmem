//! In-memory typed key/value map used by the store layer.
//!
//! This module intentionally wraps `BTreeMap` instead of exposing a raw
//! `HashMap`. Reasons:
//!
//! - deterministic iteration order
//! - stable test output
//! - predictable serialized ordering
//! - fewer surprises across platforms / hash seeds
//!
//! If performance characteristics later justify it, the internal engine can be
//! swapped behind this abstraction without changing higher layers.

use std::collections::btree_map::{IntoIter, Iter, Keys, Values};
use std::collections::BTreeMap;
use std::ops::Bound;

use crate::core::limits::MAX_ENTRY_COUNT;
use crate::error::{AgentMemoryError, NotFoundError, Result};
use crate::types::{Entry, Key, KeyPrefix, SetRequest, Value};

/// Typed in-memory store for validated keys and values.
///
/// This is a foundational internal type used by the persistent store layer.
/// It intentionally accepts only validated domain types.
///
/// Public-facing callers should usually interact with `Store` rather than this
/// lower-level type directly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryMap {
    entries: BTreeMap<Key, Value>,
}

impl MemoryMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Creates an empty map with a reserved conceptual capacity.
    ///
    /// Because `BTreeMap` does not support capacity reservation, this exists
    /// only for API symmetry and future backend flexibility.
    #[must_use]
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Returns the number of entries currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the map contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns `true` if the key exists.
    #[must_use]
    pub fn contains_key(&self, key: &Key) -> bool {
        self.entries.contains_key(key)
    }

    /// Inserts or replaces an entry.
    ///
    /// Returns the previous value if one existed.
    pub fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>> {
        self.ensure_capacity_for_insert()?;
        Ok(self.entries.insert(key, value))
    }

    /// Inserts or replaces an entry using a typed request.
    pub fn set(&mut self, request: SetRequest) -> Result<Option<Value>> {
        self.insert(request.key, request.value)
    }

    /// Returns a borrowed value for the given key.
    #[must_use]
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.entries.get(key)
    }

    /// Returns a mutable borrowed value for the given key.
    ///
    /// This is crate-level because mutating values directly bypasses some higher
    /// level store semantics.
    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        self.entries.get_mut(key)
    }

    /// Returns the value for the key or a structured not-found error.
    pub fn require(&self, key: &Key) -> Result<&Value> {
        self.get(key)
            .ok_or_else(|| AgentMemoryError::NotFound(NotFoundError::new("key", key.as_str())))
    }

    /// Removes an entry by key and returns the previous value if present.
    pub fn remove(&mut self, key: &Key) -> Option<Value> {
        self.entries.remove(key)
    }

    /// Removes an entry by key or returns a structured not-found error.
    pub fn remove_required(&mut self, key: &Key) -> Result<Value> {
        self.remove(key)
            .ok_or_else(|| AgentMemoryError::NotFound(NotFoundError::new("key", key.as_str())))
    }

    /// Returns all entries in deterministic sorted order.
    #[must_use]
    pub fn entries(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .map(|(key, value)| Entry::new(key.clone(), value.clone()))
            .collect()
    }

    /// Returns all keys in deterministic sorted order.
    #[must_use]
    pub fn keys(&self) -> Vec<Key> {
        self.entries.keys().cloned().collect()
    }

    /// Returns all values in deterministic sorted order by key.
    #[must_use]
    pub fn values(&self) -> Vec<Value> {
        self.entries.values().cloned().collect()
    }

    /// Returns all entries matching a validated prefix.
    #[must_use]
    pub fn entries_with_prefix(&self, prefix: &KeyPrefix) -> Vec<Entry> {
        self.range_for_prefix(prefix)
            .map(|(key, value)| Entry::new(key.clone(), value.clone()))
            .collect()
    }

    /// Returns all keys matching a validated prefix.
    #[must_use]
    pub fn keys_with_prefix(&self, prefix: &KeyPrefix) -> Vec<Key> {
        self.range_for_prefix(prefix)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Removes all entries matching the prefix.
    ///
    /// Returns the number of removed entries.
    pub fn remove_prefix(&mut self, prefix: &KeyPrefix) -> usize {
        let keys: Vec<Key> = self.keys_with_prefix(prefix);

        let removed = keys.len();

        for key in keys {
            let _ = self.entries.remove(&key);
        }

        removed
    }

    /// Extends the map from an iterator of typed entries.
    pub fn extend<I>(&mut self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = Entry>,
    {
        for entry in iter {
            self.insert(entry.key, entry.value)?;
        }

        Ok(())
    }

    /// Returns an iterator over entries.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, Key, Value> {
        self.entries.iter()
    }

    /// Returns an iterator over keys.
    #[must_use]
    pub fn iter_keys(&self) -> Keys<'_, Key, Value> {
        self.entries.keys()
    }

    /// Returns an iterator over values.
    #[must_use]
    pub fn iter_values(&self) -> Values<'_, Key, Value> {
        self.entries.values()
    }

    /// Returns the first key/value pair in sorted order.
    #[must_use]
    pub fn first(&self) -> Option<Entry> {
        self.entries
            .first_key_value()
            .map(|(key, value)| Entry::new(key.clone(), value.clone()))
    }

    /// Returns the last key/value pair in sorted order.
    #[must_use]
    pub fn last(&self) -> Option<Entry> {
        self.entries
            .last_key_value()
            .map(|(key, value)| Entry::new(key.clone(), value.clone()))
    }

    /// Returns statistics describing the current in-memory state.
    #[must_use]
    pub fn stats(&self) -> MapStats {
        MapStats {
            entry_count: self.len(),
            is_empty: self.is_empty(),
        }
    }

    /// Consumes the map into an owning iterator.
    #[must_use]
    pub fn into_iter_owned(self) -> IntoIter<Key, Value> {
        self.entries.into_iter()
    }

    fn ensure_capacity_for_insert(&self) -> Result<()> {
        if self.entries.len() >= MAX_ENTRY_COUNT {
            return Err(AgentMemoryError::overflow(
                "inserting beyond MAX_ENTRY_COUNT",
            ));
        }

        Ok(())
    }

    fn range_for_prefix<'a>(
        &'a self,
        prefix: &'a KeyPrefix,
    ) -> impl Iterator<Item = (&'a Key, &'a Value)> + 'a {
        let start = Bound::Included(
            Key::new(prefix.as_str())
                .expect("validated prefix must be convertible to key boundary"),
        );

        let end = Bound::Unbounded;

        self.entries
            .range((start, end))
            .take_while(move |(key, _)| prefix.matches(key))
    }
}

/// Lightweight runtime statistics for a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapStats {
    /// Number of stored entries.
    pub entry_count: usize,
    /// Whether the map is empty.
    pub is_empty: bool,
}

impl IntoIterator for MemoryMap {
    type Item = (Key, Value);
    type IntoIter = IntoIter<Key, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl FromIterator<Entry> for MemoryMap {
    fn from_iter<T: IntoIterator<Item = Entry>>(iter: T) -> Self {
        let mut map = Self::new();

        for entry in iter {
            map.entries.insert(entry.key, entry.value);
        }

        map
    }
}
