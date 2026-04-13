//! Interactive onboarding flow.
//!
//! This module powers first-run setup for `agentmem init`.
//!
//! Goals:
//!
//! - make local setup frictionless
//! - create a validated config
//! - choose a safe default storage path
//! - remain transparent about what will be written
//! - avoid hidden mutations until confirmed

use std::path::{Path, PathBuf};

use crate::cli::output::{print_blank_line, print_field, print_heading, print_info, print_success};
use crate::cli::prompts::{prompt_confirm, prompt_project_name, prompt_store_path_default};
use crate::config::{Config, ConfigDraft};
use crate::error::Result;
use crate::types::{ProjectName, StorePath};

/// Result of a successful onboarding flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingResult {
    /// Final validated config.
    pub config: Config,

    /// Path where the config file was written.
    pub config_path: PathBuf,
}

/// Runs the interactive onboarding flow.
///
/// Typical usage:
///
/// ```text
/// agentmem init
/// ```
pub fn run_onboarding() -> Result<OnboardingResult> {
    let cwd = std::env::current_dir()?;
    run_onboarding_in(&cwd)
}

/// Runs onboarding rooted at a provided project directory.
///
/// Useful for tests and future automation flows.
pub fn run_onboarding_in(project_root: &Path) -> Result<OnboardingResult> {
    print_heading("Agent Memory Setup");
    print_info("Create a local project memory store.");

    print_blank_line();

    let project_name = prompt_project_name()?;
    let default_store = Config::project_store_path(project_root);
    let store_path = prompt_store_path_default(default_store)?;

    let draft = ConfigDraft::new(project_name.clone(), store_path.clone());

    print_blank_line();
    preview(&project_name, &store_path, project_root);

    let confirmed = prompt_confirm("Create configuration now?", true)?;

    if !confirmed {
        return Err(crate::error::AgentMemoryError::internal(
            "onboarding cancelled by user",
        ));
    }

    let config = draft.finalize()?;
    let config_path = Config::project_config_path(project_root);

    config.save(&config_path)?;

    print_blank_line();
    print_success("Configuration created.");
    print_field("Config file", &config_path.display().to_string());
    print_field("Store file", &store_path.to_string());

    Ok(OnboardingResult {
        config,
        config_path,
    })
}

/// Prints a dry-run preview before writing files.
fn preview(project_name: &ProjectName, store_path: &StorePath, project_root: &Path) {
    let config_path = Config::project_config_path(project_root);

    print_heading("Preview");
    print_field("Project", project_name.as_str());
    print_field("Config file", &config_path.display().to_string());
    print_field("Store file", &store_path.to_string());
}
