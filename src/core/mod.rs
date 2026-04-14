//! Core domain primitives and validation logic.
//!
//! This module contains the low-level building blocks that the rest of the
//! crate relies on:
//!
//! - hard limits that define safe operational boundaries
//! - validation rules for keys, namespaces, values, project names, and paths
//! - namespace helpers for prefix-aware key operations
//! - the in-memory map abstraction used by the store layer
//!
//! The intent is to keep these concerns centralized so that:
//!
//! - validation policy is not duplicated across the codebase
//! - limits remain explicit and reviewable
//! - typed wrappers in `types.rs` delegate to a single source of truth
//! - future storage model changes remain controlled and auditable

pub mod limits;
pub mod map;
pub mod namespace;
pub mod validation;

/// Re-export of the in-memory map type used by the store layer.
pub use self::map::MemoryMap;

/// Re-export of commonly used namespace helpers.
pub use self::namespace::{
    is_key_within_namespace,
    join_namespace_and_leaf,
    parent_namespace,
};

/// Re-export of canonical validation entrypoints.
pub use self::validation::{
    validate_key,
    validate_key_leaf,
    validate_namespace,
    validate_project_name,
    validate_store_path,
    validate_value,
};