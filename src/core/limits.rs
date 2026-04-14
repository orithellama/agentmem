//! Centralized hard limits for the crate.
//!
//! These constants define the operational boundaries for keys, namespaces,
//! values, indexing payloads, configuration fields and storage parsing.
//!
//! Keeping limits in one place makes them:
//!
//! - easy to audit
//! - easy to test
//! - harder to accidentally drift across modules
//! - explicit for security review
//!
//! These values are conservative but practical for real local project indexing.
//! They may evolve over time, but should not change casually because they affect:
//!
//! - validation behavior
//! - parser hardening
//! - denial-of-service resistance
//! - storage compatibility expectations
//! - memory usage characteristics

// --------------------------------------------------
// Keys / namespaces
// --------------------------------------------------

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
/// Protects against abusive deeply nested path inputs.
pub const MAX_SEGMENT_COUNT: usize = 32;

/// Maximum number of bytes allowed in a namespace.
pub const MAX_NAMESPACE_LEN: usize = 384;

/// Minimum number of bytes required for a namespace.
pub const MIN_NAMESPACE_LEN: usize = 1;

// --------------------------------------------------
// Project / config
// --------------------------------------------------

/// Maximum number of bytes allowed in a project name.
pub const MAX_PROJECT_NAME_LEN: usize = 128;

/// Minimum number of bytes required for a project name.
pub const MIN_PROJECT_NAME_LEN: usize = 1;

/// Maximum number of bytes allowed in a fully resolved store path string.
pub const MAX_STORE_PATH_LEN: usize = 4096;

/// Maximum number of bytes allowed in a config file payload.
pub const MAX_CONFIG_FILE_LEN: usize = 64 * 1024; // 64 KiB

/// Maximum bytes allowed in the primary store file name.
pub const MAX_STORE_FILE_NAME_LEN: usize = 255;

/// Maximum bytes allowed in a rendered storage format version field.
pub const MAX_VERSION_FIELD_LEN: usize = 16;

// --------------------------------------------------
// Values / storage
// --------------------------------------------------

/// Maximum number of bytes allowed in a generic stored value.
///
/// Supports indexing payloads, posting lists, chunk records,
/// metadata summaries and normal memory values.
pub const MAX_VALUE_LEN: usize = 512 * 1024; // 512 KiB

/// Minimum number of bytes required for a value.
pub const MIN_VALUE_LEN: usize = 0;

/// Maximum number of bytes allowed in a single serialized line of the storage file.
///
/// Must safely exceed MAX_VALUE_LEN once escaped/encoded.
pub const MAX_STORE_LINE_LEN: usize = 1024 * 1024; // 1 MiB

/// Maximum number of entries allowed in one in-memory map instance.
pub const MAX_ENTRY_COUNT: usize = 1_000_000;

// --------------------------------------------------
// Hashmap tuning
// --------------------------------------------------

/// Default initial capacity for a newly created in-memory map.
pub const DEFAULT_MAP_CAPACITY: usize = 64;

/// Minimum valid capacity for the in-memory map.
pub const MIN_MAP_CAPACITY: usize = 16;

/// Maximum load factor before the in-memory map should resize.
pub const MAX_LOAD_FACTOR: f64 = 0.70;

// --------------------------------------------------
// Indexing / repository intelligence
// --------------------------------------------------

/// Maximum file size eligible for indexing.
///
/// Files above this threshold should be metadata-only or streamed later.
pub const INDEX_MAX_FILE_BYTES: usize = 2_500 * 1024; // 2.5 MB

/// Target maximum lines per chunk.
pub const INDEX_CHUNK_LINE_TARGET: usize = 40;

/// Target maximum bytes per chunk.
pub const INDEX_CHUNK_BYTE_TARGET: usize = 3_500;

/// Maximum bytes for one persisted text chunk.
pub const INDEX_MAX_TEXT_CHUNK_LEN: usize = 256 * 1024; // 256 KiB

/// Maximum bytes for metadata-only asset records.
pub const INDEX_METADATA_SUMMARY_MAX_LEN: usize = 64 * 1024; // 64 KiB

/// Maximum number of posting references stored per token.
pub const INDEX_MAX_POSTINGS_PER_TOKEN: usize = 256;

/// Maximum bytes for serialized posting lists.
pub const INDEX_MAX_POSTING_LIST_LEN: usize = 512 * 1024; // 512 KiB

/// Maximum assets scanned in one indexing pass.
pub const INDEX_MAX_ASSET_SCAN_COUNT: usize = 250_000;

/// Maximum file path length persisted in index metadata.
pub const INDEX_MAX_PATH_LEN: usize = 2048;

/// Maximum image dimension accepted in metadata sanity checks.
pub const INDEX_MAX_IMAGE_DIMENSION: u32 = 50_000;

// --------------------------------------------------
// Retrieval / query
// --------------------------------------------------

/// Default query chunks returned.
pub const DEFAULT_QUERY_TOP_K: usize = 8;

/// Maximum query chunks returned.
pub const INDEX_MAX_TOP_K: usize = 64;

/// Minimum retrieval token budget.
pub const INDEX_MIN_TOKEN_BUDGET: usize = 128;

/// Default retrieval token budget.
pub const DEFAULT_QUERY_TOKEN_BUDGET: usize = 4_000;

/// Maximum retrieval token budget.
pub const INDEX_MAX_TOKEN_BUDGET: usize = 64_000;

// --------------------------------------------------
// Tokenization
// --------------------------------------------------

/// Minimum indexed token length.
pub const INDEX_MIN_TOKEN_LEN: usize = 2;

/// Maximum indexed token length.
pub const INDEX_MAX_TOKEN_LEN: usize = 40;

// --------------------------------------------------
// Helper
// --------------------------------------------------

/// Returns true if the provided length is within the inclusive range.
#[must_use]
pub const fn within_range(len: usize, min: usize, max: usize) -> bool {
    len >= min && len <= max
}