//! Centralized hard limits for the crate.
//!
//! These constants define the operational boundaries for keys, namespaces,
//! values, configuration fields and storage parsing.
//!
//! Keeping limits in one place makes them:
//!
//! - easy to audit
//! - easy to test
//! - harder to accidentally drift across modules
//! - explicit for security review
//!
//! These values are intentionally conservative for v1. They can be revisited
//! later, but they should not change casually because they affect:
//!
//! - validation behavior
//! - parser hardening
//! - denial-of-service resistance
//! - storage compatibility expectations

/// Maximum number of bytes allowed in a key.
///
/// Keys should remain short, human-readable and namespace-oriented.
pub const MAX_KEY_LEN: usize = 512;

/// Minimum number of bytes required for a key.
pub const MIN_KEY_LEN: usize = 1;

/// Maximum number of bytes allowed in a single key segment.
///
/// Example segments:
///
/// - `agent`
/// - `claude`
/// - `current_task`
pub const MAX_KEY_SEGMENT_LEN: usize = 128;

/// Minimum number of bytes required for a key segment.
pub const MIN_KEY_SEGMENT_LEN: usize = 1;

/// Maximum number of namespace segments allowed in a key or namespace.
///
/// This protects against deeply nested path-like structures that are difficult
/// to reason about and may be abused for oversized inputs.
pub const MAX_SEGMENT_COUNT: usize = 32;

/// Maximum number of bytes allowed in a namespace.
pub const MAX_NAMESPACE_LEN: usize = 384;

/// Minimum number of bytes required for a namespace.
pub const MIN_NAMESPACE_LEN: usize = 1;

/// Maximum number of bytes allowed in a project name.
///
/// Project names should remain short enough to display clearly and map
/// comfortably into namespaces and filesystem suggestions.
pub const MAX_PROJECT_NAME_LEN: usize = 128;

/// Minimum number of bytes required for a project name.
pub const MIN_PROJECT_NAME_LEN: usize = 1;

/// Maximum number of bytes allowed in a value.
///
/// This is intentionally bounded in v1 to avoid unbounded memory growth and to
/// keep the local store focused on agent state rather than arbitrary document
/// storage.
pub const MAX_VALUE_LEN: usize = 64 * 1024; // 64 KiB

/// Minimum number of bytes required for a value.
///
/// Empty values are allowed in v1 because explicit emptiness can be meaningful
/// in agent workflows.
pub const MIN_VALUE_LEN: usize = 0;

/// Maximum number of bytes allowed in a single serialized line of the storage file.
///
/// This limit protects the parser from unbounded line growth.
pub const MAX_STORE_LINE_LEN: usize = 128 * 1024; // 128 KiB

/// Maximum number of entries allowed in a single in-memory map instance.
///
/// This is a safety boundary, not a promise that every deployment should aim
/// for this scale.
pub const MAX_ENTRY_COUNT: usize = 1_000_000;

/// Maximum number of bytes allowed in a fully resolved store path string.
///
/// This is a policy limit to catch unreasonable inputs early.
pub const MAX_STORE_PATH_LEN: usize = 4096;

/// Maximum number of bytes allowed in a config file payload.
///
/// The config format is intentionally small; anything significantly larger is
/// likely malformed or a misuse of the file.
pub const MAX_CONFIG_FILE_LEN: usize = 64 * 1024; // 64 KiB

/// Default initial capacity for a newly created in-memory map.
///
/// This should be large enough to avoid immediate resizing for small projects
/// without wasting excessive memory.
pub const DEFAULT_MAP_CAPACITY: usize = 64;

/// Minimum valid capacity for the in-memory map.
///
/// Internal map code may round capacities upward depending on its design.
pub const MIN_MAP_CAPACITY: usize = 16;

/// Maximum load factor before the in-memory map should resize.
///
/// The map implementation may use this threshold to balance memory overhead and
/// lookup performance.
pub const MAX_LOAD_FACTOR: f64 = 0.70;

/// Maximum number of bytes allowed in a storage format version field once rendered.
///
/// This is mainly useful for parser hardening and defensive checks.
pub const MAX_VERSION_FIELD_LEN: usize = 16;

/// Maximum number of bytes allowed in a file name used for the primary store.
///
/// This does not replace filesystem rules; it is a crate-level sanity limit.
pub const MAX_STORE_FILE_NAME_LEN: usize = 255;

/// Returns `true` if the provided length is within the inclusive range.
///
/// This helper keeps validation sites a little cleaner.
#[must_use]
pub const fn within_range(len: usize, min: usize, max: usize) -> bool {
    len >= min && len <= max
}
