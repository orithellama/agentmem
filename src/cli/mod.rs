//! Command-line interface support.
//!
//! This module contains reusable CLI-facing logic used by binaries such as:
//!
//! - `agentmem`   (interactive/local CLI)
//! - `agentmemd`  (future daemon/service mode)
//!
//! Design goals:
//!
//! - keep binary entrypoints thin
//! - separate command parsing from business logic
//! - centralize human-readable output
//! - support machine-friendly output later
//! - make onboarding flows testable

pub mod commands;
pub mod onboarding;
pub mod output;
pub mod prompts;

/// Re-export of the primary command runner.
///
/// Intended use in binaries:
///
/// ```rust,no_run
/// use agent_hashmap::cli::run;
/// ```
pub use self::commands::run;

/// Re-export common output helpers.
pub use self::output::{print_error, print_info, print_success, print_warning};

/// Re-export onboarding entrypoint.
pub use self::onboarding::run_onboarding;
