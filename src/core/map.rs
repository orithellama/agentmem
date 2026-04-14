//! In-memory typed key/value map used by the store layer.
//!
//! This module wraps `BTreeMap` instead of exposing a raw `HashMap`.
//!
//! Reasons:
//!
//! - deterministic iteration order
//! - stable test output
//! - predictable serialized ordering
//! - fewer surprises across platforms
//!
//! If needed later, the backend can be swapped behind this abstraction.

use std::collections::btree_map::{
    IntoIter,
    Iter,
    Keys,
    Values,
};
use std::collections::BTreeMap;
use std::ops::Bound;

use crate::core::limits::MAX_ENTRY_COUNT;
use crate::error::{
    AgentMemoryError,
    NotFoundError,
    Result,
};
use crate::types::{
    Entry,
    Key,
    KeyPrefix,
    SetRequest,
    Value,
};

/// Typed in-memory store for validated keys and values.
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

    /// Creates an empty map with conceptual capacity.
    ///
    /// `BTreeMap` does not reserve capacity, but this keeps API symmetry.
    #[must_use]
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// True if key exists.
    #[must_use]
    pub fn contains_key(
        &self,
        key: &Key,
    ) -> bool {
        self.entries.contains_key(key)
    }

    /// Inserts or replaces a value.
    pub fn insert(
        &mut self,
        key: Key,
        value: Value,
    ) -> Result<Option<Value>> {
        self.ensure_capacity_for_insert()?;
        Ok(self.entries.insert(key, value))
    }

    /// Inserts from typed request.
    pub fn set(
        &mut self,
        request: SetRequest,
    ) -> Result<Option<Value>> {
        self.insert(request.key, request.value)
    }

    /// Gets borrowed value.
    #[must_use]
    pub fn get(
        &self,
        key: &Key,
    ) -> Option<&Value> {
        self.entries.get(key)
    }

    /// Gets mutable borrowed value.
    pub(crate) fn get_mut(
        &mut self,
        key: &Key,
    ) -> Option<&mut Value> {
        self.entries.get_mut(key)
    }

    /// Requires a value or returns not-found error.
    pub fn require(
        &self,
        key: &Key,
    ) -> Result<&Value> {
        self.get(key).ok_or_else(|| {
            AgentMemoryError::NotFound(
                NotFoundError::new(
                    "key",
                    key.as_str(),
                ),
            )
        })
    }

    /// Removes a key.
    pub fn remove(
        &mut self,
        key: &Key,
    ) -> Option<Value> {
        self.entries.remove(key)
    }

    /// Removes a key or errors.
    pub fn remove_required(
        &mut self,
        key: &Key,
    ) -> Result<Value> {
        self.remove(key).ok_or_else(|| {
            AgentMemoryError::NotFound(
                NotFoundError::new(
                    "key",
                    key.as_str(),
                ),
            )
        })
    }

    /// All entries in sorted order.
    #[must_use]
    pub fn entries(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .map(|(k, v)| {
                Entry::new(k.clone(), v.clone())
            })
            .collect()
    }

    /// All keys in sorted order.
    #[must_use]
    pub fn keys(&self) -> Vec<Key> {
        self.entries.keys().cloned().collect()
    }

    /// All values in sorted key order.
    #[must_use]
    pub fn values(&self) -> Vec<Value> {
        self.entries.values().cloned().collect()
    }

    /// Entries matching prefix.
    #[must_use]
    pub fn entries_with_prefix(
        &self,
        prefix: &KeyPrefix,
    ) -> Vec<Entry> {
        self.range_for_prefix(prefix)
            .map(|(k, v)| {
                Entry::new(k.clone(), v.clone())
            })
            .collect()
    }

    /// Keys matching prefix.
    #[must_use]
    pub fn keys_with_prefix(
        &self,
        prefix: &KeyPrefix,
    ) -> Vec<Key> {
        self.range_for_prefix(prefix)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Removes all matching prefix entries.
    ///
    /// Returns removed count.
    pub fn remove_prefix(
        &mut self,
        prefix: &KeyPrefix,
    ) -> usize {
        let keys = self.keys_with_prefix(prefix);
        let removed = keys.len();

        for key in keys {
            let _ = self.entries.remove(&key);
        }

        removed
    }

    /// Extends map from entries.
    pub fn extend<I>(
        &mut self,
        iter: I,
    ) -> Result<()>
    where
        I: IntoIterator<Item = Entry>,
    {
        for entry in iter {
            self.insert(entry.key, entry.value)?;
        }

        Ok(())
    }

    /// Iterator over entries.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, Key, Value> {
        self.entries.iter()
    }

    /// Iterator over keys.
    #[must_use]
    pub fn iter_keys(
        &self,
    ) -> Keys<'_, Key, Value> {
        self.entries.keys()
    }

    /// Iterator over values.
    #[must_use]
    pub fn iter_values(
        &self,
    ) -> Values<'_, Key, Value> {
        self.entries.values()
    }

    /// First sorted entry.
    #[must_use]
    pub fn first(&self) -> Option<Entry> {
        self.entries
            .first_key_value()
            .map(|(k, v)| {
                Entry::new(k.clone(), v.clone())
            })
    }

    /// Last sorted entry.
    #[must_use]
    pub fn last(&self) -> Option<Entry> {
        self.entries
            .last_key_value()
            .map(|(k, v)| {
                Entry::new(k.clone(), v.clone())
            })
    }

    /// Runtime statistics.
    #[must_use]
    pub fn stats(&self) -> MapStats {
        MapStats {
            entry_count: self.len(),
            is_empty: self.is_empty(),
        }
    }

    /// Consumes into owning iterator.
    #[must_use]
    pub fn into_iter_owned(
        self,
    ) -> IntoIter<Key, Value> {
        self.entries.into_iter()
    }

    fn ensure_capacity_for_insert(
        &self,
    ) -> Result<()> {
        if self.entries.len() >= MAX_ENTRY_COUNT {
            return Err(
                AgentMemoryError::overflow(
                    "inserting beyond MAX_ENTRY_COUNT",
                ),
            );
        }

        Ok(())
    }

    fn range_for_prefix<'a>(
        &'a self,
        prefix: &'a KeyPrefix,
    ) -> impl Iterator<
        Item = (&'a Key, &'a Value),
    > + 'a {
        let start = Bound::Included(
            Key::new(prefix.as_str())
                .expect(
                    "validated prefix must convert to key boundary",
                ),
        );

        self.entries
            .range((start, Bound::Unbounded))
            .take_while(move |(key, _)| {
                prefix.matches(key)
            })
    }
}

/// Lightweight runtime stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapStats {
    pub entry_count: usize,
    pub is_empty: bool,
}

impl IntoIterator for MemoryMap {
    type Item = (Key, Value);
    type IntoIter = IntoIter<Key, Value>;

    fn into_iter(
        self,
    ) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl FromIterator<Entry> for MemoryMap {
    fn from_iter<T>(
        iter: T,
    ) -> Self
    where
        T: IntoIterator<Item = Entry>,
    {
        let mut map = Self::new();

        for entry in iter {
            map.entries.insert(
                entry.key,
                entry.value,
            );
        }

        map
    }
}