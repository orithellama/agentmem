//! Interactive prompt helpers for CLI flows.
//!
//! This module centralizes user input collection for onboarding and destructive
//! confirmations.

use std::io::{self, Write};
use std::path::PathBuf;

use crate::error::{AgentMemoryError, Result, ValidationError};
use crate::types::{ProjectName, StorePath};

/// Prompts the user for a project name until a valid value is entered.
pub fn prompt_project_name() -> Result<ProjectName> {
    loop {
        let raw = prompt_line("Project name: ")?;
        let candidate = raw.trim();

        match ProjectName::new(candidate.to_owned()) {
            Ok(value) => return Ok(value),
            Err(error) => eprintln!("[error] {error}"),
        }
    }
}

/// Prompts the user for a store path, allowing an empty response to accept
/// `default_path`.
pub fn prompt_store_path_default(default_path: PathBuf) -> Result<StorePath> {
    let prompt = format!("Store path [{}]: ", default_path.display());

    loop {
        let raw = prompt_line(&prompt)?;
        let candidate = raw.trim();

        let resolved = if candidate.is_empty() {
            default_path.clone()
        } else {
            PathBuf::from(candidate)
        };

        match StorePath::new(resolved) {
            Ok(path) => return Ok(path),
            Err(error) => eprintln!("[error] {error}"),
        }
    }
}

/// Prompts for yes/no confirmation.
///
/// Empty input selects `default`.
pub fn prompt_confirm(question: &str, default: bool) -> Result<bool> {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    let prompt = format!("{question} {suffix}: ");

    loop {
        let raw = prompt_line(&prompt)?;
        let normalized = raw.trim().to_ascii_lowercase();

        if normalized.is_empty() {
            return Ok(default);
        }

        if matches!(normalized.as_str(), "y" | "yes") {
            return Ok(true);
        }

        if matches!(normalized.as_str(), "n" | "no") {
            return Ok(false);
        }

        eprintln!("[warn] please answer yes or no");
    }
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().map_err(AgentMemoryError::from)?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(AgentMemoryError::from)?;

    if input.contains('\0') {
        return Err(
            ValidationError::invalid_format("prompt_input", "must not contain NUL bytes").into(),
        );
    }

    Ok(input)
}
