//! Durable store layer.
//!
//! This module provides the high-level persistent memory engine used by both
//! library consumers and the CLI.
//!
//! Responsibilities:
//!
//! - own the in-memory map
//! - coordinate persistence to disk
//! - apply locking semantics
//! - expose typed CRUD operations
//! - keep runtime configuration attached to the store
//! - hide lower-level file format details behind stable APIs
//!
//! The store layer is intentionally separated from `core`:
//!
//! - `core` = pure logic, validation, map internals
//! - `store` = stateful runtime engine + persistence boundaries

pub mod engine;
pub mod journal;
pub mod locking;
pub mod migration;
pub mod persist;

/// Primary high-level store type.
///
/// Re-exported so callers can use:
///
/// ```rust,no_run
/// use agent_hashmap::Store;
/// ```
pub use self::engine::Store;

/// Store metadata and statistics.
pub use self::engine::{StoreInfo, StoreStats};

/// File lock guard used internally and optionally surfaced later.
pub use self::locking::StoreLock;

/// Persistence helpers and on-disk representations.
///
/// Re-exported selectively for testing and controlled advanced use.
pub use self::persist::{StoreFile, StoreRecord};

/// Current storage format version.
pub use self::migration::STORE_FORMAT_VERSION;
