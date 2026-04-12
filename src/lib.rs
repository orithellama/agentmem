#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::cargo,
    clippy::pedantic,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls
)]
#![allow(clippy::module_name_repetitions)]

//! Agent Memory RS
//!
//! A secure, local-first memory layer for AI agents.
//!
//! This crate is designed to provide a predictable, typed, auditable API for:
//!
//! - project-scoped local memory
//! - namespaced agent state
//! - durable on-disk persistence
//! - strict validation of keys and paths
//! - future-safe extension toward multi-agent workflows
//!
//! # Module layout
//!
//! - [`error`] contains crate-wide error types
//! - [`types`] contains domain-specific typed wrappers
//! - [`config`] contains configuration loading and validation
//!! - [`core`] contains foundational validation and namespace logic
//! - [`store`] contains the durable storage engine
//! - [`cli`] contains CLI-facing logic used by binaries
//!
//! # Stability note
//!
//! This crate is in early development. The goal is to make the *behavior*
//! predictable before making the public API large.

pub mod cli;
pub mod config;
pub mod core;
pub mod error;
pub mod store;
pub mod types;

/// Re-export of the crate's primary error type.
///
/// This keeps call sites ergonomic:
///
/// ```rust,no_run
/// use agent_hashmap::Result;
/// ```
///
/// rather than forcing every consumer to spell the full error path.
pub use crate::error::{AgentMemoryError, Result};

/// Re-export of the high-level store type.
///
/// This is intended to become the main entrypoint for both library consumers
/// and internal CLI flows.
pub use crate::store::Store;

/// Re-export of frequently used typed domain values.
///
/// These wrappers exist to keep the API explicit and harder to misuse than
/// passing around unvalidated raw strings everywhere.
pub use crate::types::{Key, Namespace, ProjectName, StorePath, Value};