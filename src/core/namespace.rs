//! Namespace and key path helpers.
//!
//! This module contains pure helpers for manipulating namespace-style keys.
//! Keeping this logic separate from `types.rs` helps maintain:
//!
//! - testable string/path semantics
//! - no duplication across CLI and store code
//! - predictable prefix behavior
//! - easier future migration rules
//!
//! Examples of supported paths:
//!
//! - `agent/claude/current_task`
//! - `project/demo/root`
//! - `session/2026-04-12/state`

use crate::error::Result;
use crate::types::{Key, KeyPrefix, Namespace};

/// Returns `true` if a key belongs to the provided namespace.
///
/// Matching rules:
///
/// - exact namespace match is allowed
/// - nested descendants are allowed
/// - partial segment matches are not allowed
///
/// Examples:
///
/// namespace: `agent/claude`
///
/// matches:
/// - `agent/claude`
/// - `agent/claude/current_task`
///
/// does not match:
/// - `agent/claude2`
/// - `agent/cla`
#[must_use]
pub fn is_key_within_namespace(key: &Key, namespace: &Namespace) -> bool {
    let key_str = key.as_str();
    let ns = namespace.as_str();

    if key_str == ns {
        return true;
    }

    key_str
        .strip_prefix(ns)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

/// Joins a namespace and leaf segment into a validated key.
///
/// Example:
///
/// namespace: `agent/claude`
/// leaf: `current_task`
///
/// result:
///
/// `agent/claude/current_task`
pub fn join_namespace_and_leaf(namespace: &Namespace, leaf: &str) -> Result<Key> {
    namespace.join(leaf)
}

/// Returns the parent namespace, if one exists.
///
/// Examples:
///
/// - `agent/claude` -> `agent`
/// - `project/demo/config` -> `project/demo`
/// - `agent` -> `None`
#[must_use]
pub fn parent_namespace(namespace: &Namespace) -> Option<Namespace> {
    namespace.parent()
}

/// Returns all ancestor namespaces from nearest to root.
///
/// Example:
///
/// `agent/claude/tasks` ->
///
/// - `agent/claude`
/// - `agent`
#[must_use]
pub fn namespace_ancestors(namespace: &Namespace) -> Vec<Namespace> {
    let mut current = namespace.clone();
    let mut result = Vec::new();

    while let Some(parent) = current.parent() {
        result.push(parent.clone());
        current = parent;
    }

    result
}

/// Returns the depth (segment count) of a namespace.
///
/// Examples:
///
/// - `agent` => 1
/// - `agent/claude` => 2
/// - `agent/claude/tasks` => 3
#[must_use]
pub fn namespace_depth(namespace: &Namespace) -> usize {
    segment_count(namespace.as_str())
}

/// Returns the depth (segment count) of a key.
///
/// Examples:
///
/// - `agent/claude/current_task` => 3
/// - `project/demo/root` => 3
#[must_use]
pub fn key_depth(key: &Key) -> usize {
    segment_count(key.as_str())
}

/// Returns the leaf segment of a namespace.
///
/// Example:
///
/// `agent/claude` -> `claude`
#[must_use]
pub fn namespace_leaf(namespace: &Namespace) -> &str {
    namespace.leaf()
}

/// Returns the leaf segment of a key.
///
/// Example:
///
/// `agent/claude/current_task` -> `current_task`
#[must_use]
pub fn key_leaf(key: &Key) -> &str {
    key.leaf()
}

/// Converts a namespace into a prefix matcher.
///
/// Useful for listing keys under a namespace.
#[must_use]
pub fn namespace_prefix(namespace: &Namespace) -> KeyPrefix {
    KeyPrefix::new(namespace.as_str())
        .expect("validated namespace must always produce a valid prefix")
}

/// Returns `true` if the key matches the provided prefix.
#[must_use]
pub fn key_matches_prefix(key: &Key, prefix: &KeyPrefix) -> bool {
    prefix.matches(key)
}

/// Splits a key into `(namespace, leaf)` if it has at least one separator.
///
/// Examples:
///
/// - `agent/claude/current_task` -> (`agent/claude`, `current_task`)
/// - `agent` -> `None`
#[must_use]
pub fn split_key(key: &Key) -> Option<(Namespace, String)> {
    let raw = key.as_str();

    raw.rsplit_once('/').map(|(prefix, leaf)| {
        (
            Namespace::new(prefix).expect("validated key prefix must always be a valid namespace"),
            leaf.to_owned(),
        )
    })
}

/// Returns the common namespace prefix shared by two keys.
///
/// Examples:
///
/// - `agent/claude/task` + `agent/claude/state` => `agent/claude`
/// - `agent/claude/task` + `agent/codex/task` => `agent`
/// - `agent/x` + `project/y` => `None`
#[must_use]
pub fn common_namespace(left: &Key, right: &Key) -> Option<Namespace> {
    let left_parts: Vec<&str> = left.as_str().split('/').collect();
    let right_parts: Vec<&str> = right.as_str().split('/').collect();

    let mut shared = Vec::new();

    for (a, b) in left_parts.iter().zip(right_parts.iter()) {
        if a == b {
            shared.push(*a);
        } else {
            break;
        }
    }

    if shared.is_empty() {
        return None;
    }

    let joined = shared.join("/");

    Namespace::new(joined).ok()
}

/// Returns a normalized path-like string by trimming redundant surrounding `/`.
///
/// This helper is intentionally conservative. It does not rewrite internal
/// segments or collapse repeated separators.
#[must_use]
pub fn trim_outer_separators(input: &str) -> &str {
    input.trim_matches('/')
}

/// Counts path segments in a validated key or namespace string.
#[must_use]
fn segment_count(input: &str) -> usize {
    input.split('/').count()
}
