//! Canonical validation rules for all externally supplied input.
//!
//! This module is the single source of truth for validating:
//!
//! - keys
//! - namespaces
//! - key leaf segments
//! - values
//! - project names
//! - store paths
//!
//! Centralizing these rules prevents drift across the codebase and keeps typed
//! wrappers (`Key`, `Namespace`, `ProjectName`, `StorePath`, `Value`) aligned.

use std::path::{Component, Path};

use crate::core::limits::{
    within_range,
    MAX_KEY_LEN,
    MIN_KEY_LEN,
    MAX_KEY_SEGMENT_LEN,
    MIN_KEY_SEGMENT_LEN,
    MAX_NAMESPACE_LEN,
    MIN_NAMESPACE_LEN,
    MAX_PROJECT_NAME_LEN,
    MIN_PROJECT_NAME_LEN,
    MAX_SEGMENT_COUNT,
    MAX_STORE_PATH_LEN,
    MAX_STORE_FILE_NAME_LEN,
    MAX_VALUE_LEN,
    MIN_VALUE_LEN,
};

use crate::error::{Result, ValidationError};

/// Validates a fully qualified key.
///
/// Examples:
///
/// - `agent/claude/current_task`
/// - `project/demo/root`
/// - `session/2026-04-12/state`
pub fn validate_key(input: &str) -> Result<()> {
    validate_common_text("key", input, MIN_KEY_LEN, MAX_KEY_LEN)?;

    reject_edge_slashes("key", input)?;
    reject_empty_segments("key", input)?;

    let segments: Vec<&str> = input.split('/').collect();

    if segments.len() > MAX_SEGMENT_COUNT {
        return Err(
            ValidationError::too_long("key_segments", segments.len(), MAX_SEGMENT_COUNT).into(),
        );
    }

    for segment in segments {
        validate_segment("key", segment)?;
    }

    Ok(())
}

/// Validates a namespace.
///
/// Examples:
///
/// - `agent/claude`
/// - `project/demo`
/// - `session/2026-04-12`
pub fn validate_namespace(input: &str) -> Result<()> {
    validate_common_text("namespace", input, MIN_NAMESPACE_LEN, MAX_NAMESPACE_LEN)?;

    reject_edge_slashes("namespace", input)?;
    reject_empty_segments("namespace", input)?;

    let segments: Vec<&str> = input.split('/').collect();

    if segments.len() > MAX_SEGMENT_COUNT {
        return Err(
            ValidationError::too_long(
                "namespace_segments",
                segments.len(),
                MAX_SEGMENT_COUNT,
            )
            .into(),
        );
    }

    for segment in segments {
        validate_segment("namespace", segment)?;
    }

    Ok(())
}

/// Validates a leaf key segment.
///
/// Examples:
///
/// - `current_task`
/// - `summary`
/// - `run-001`
pub fn validate_key_leaf(input: &str) -> Result<()> {
    validate_segment("key_leaf", input)
}

/// Validates a stored value.
///
/// Values are UTF-8 text in v1.
pub fn validate_value(input: &str) -> Result<()> {
    validate_common_text("value", input, MIN_VALUE_LEN, MAX_VALUE_LEN)?;

    if input.contains('\0') {
        return Err(
            ValidationError::invalid_format("value", "must not contain NUL bytes").into(),
        );
    }

    Ok(())
}

/// Validates a project name.
pub fn validate_project_name(input: &str) -> Result<()> {
    validate_common_text(
        "project_name",
        input,
        MIN_PROJECT_NAME_LEN,
        MAX_PROJECT_NAME_LEN,
    )?;

    for (index, ch) in input.char_indices() {
        if is_project_name_char(ch) {
            continue;
        }

        return Err(
            ValidationError::InvalidCharacter {
                field: "project_name",
                character: ch,
                index,
            }
            .into(),
        );
    }

    if input.starts_with('-') || input.ends_with('-') {
        return Err(
            ValidationError::invalid_format(
                "project_name",
                "must not start or end with '-'",
            )
            .into(),
        );
    }

    Ok(())
}

/// Validates a store path.
pub fn validate_store_path(path: &Path) -> Result<()> {
    let rendered = path.to_string_lossy();

    if rendered.is_empty() {
        return Err(ValidationError::empty("store_path").into());
    }

    if rendered.len() > MAX_STORE_PATH_LEN {
        return Err(
            ValidationError::too_long(
                "store_path",
                rendered.len(),
                MAX_STORE_PATH_LEN,
            )
            .into(),
        );
    }

    let file_name = path.file_name().ok_or_else(|| {
        ValidationError::invalid_path(
            "store_path",
            "path must include a file name",
        )
    })?;

    let file_name = file_name.to_string_lossy();

    if file_name.trim().is_empty() {
        return Err(
            ValidationError::invalid_path(
                "store_path",
                "file name must not be empty",
            )
            .into(),
        );
    }

    if file_name.len() > MAX_STORE_FILE_NAME_LEN {
        return Err(
            ValidationError::too_long(
                "store_file_name",
                file_name.len(),
                MAX_STORE_FILE_NAME_LEN,
            )
            .into(),
        );
    }

    for component in path.components() {
        validate_path_component(component)?;
    }

    Ok(())
}

/// Validates one namespace/key segment.
fn validate_segment(field: &'static str, segment: &str) -> Result<()> {
    validate_common_text(
        field,
        segment,
        MIN_KEY_SEGMENT_LEN,
        MAX_KEY_SEGMENT_LEN,
    )?;

    if segment == "." || segment == ".." {
        return Err(
            ValidationError::invalid_segment(
                field,
                "reserved segment is not allowed",
            )
            .into(),
        );
    }

    for (index, ch) in segment.char_indices() {
        if is_segment_char(ch) {
            continue;
        }

        return Err(
            ValidationError::InvalidCharacter {
                field,
                character: ch,
                index,
            }
            .into(),
        );
    }

    Ok(())
}

/// Shared UTF-8 bounded text validation.
fn validate_common_text(
    field: &'static str,
    input: &str,
    min_len: usize,
    max_len: usize,
) -> Result<()> {
    let len = input.len();

    if len == 0 && min_len > 0 {
        return Err(ValidationError::empty(field).into());
    }

    if !within_range(len, min_len, max_len) {
        if len < min_len {
            return Err(
                ValidationError::too_short(field, len, min_len).into(),
            );
        }

        return Err(
            ValidationError::too_long(field, len, max_len).into(),
        );
    }

    if input.contains('\0') {
        return Err(
            ValidationError::invalid_format(
                field,
                "must not contain NUL bytes",
            )
            .into(),
        );
    }

    Ok(())
}

fn reject_edge_slashes(field: &'static str, input: &str) -> Result<()> {
    if input.starts_with('/') || input.ends_with('/') {
        return Err(
            ValidationError::invalid_format(
                field,
                "must not start or end with '/'",
            )
            .into(),
        );
    }

    Ok(())
}

fn reject_empty_segments(field: &'static str, input: &str) -> Result<()> {
    if input.contains("//") {
        return Err(
            ValidationError::invalid_format(
                field,
                "must not contain empty path segments",
            )
            .into(),
        );
    }

    Ok(())
}

/// Validates path components.
fn validate_path_component(component: Component<'_>) -> Result<()> {
    match component {
        Component::ParentDir => Err(
            ValidationError::invalid_path(
                "store_path",
                "parent traversal ('..') is not allowed",
            )
            .into(),
        ),

        Component::Normal(name) => {
            let rendered = name.to_string_lossy();

            if rendered.trim().is_empty() {
                return Err(
                    ValidationError::invalid_path(
                        "store_path",
                        "path segment must not be empty",
                    )
                    .into(),
                );
            }

            if rendered == "." || rendered == ".." {
                return Err(
                    ValidationError::invalid_path(
                        "store_path",
                        "reserved path segment is not allowed",
                    )
                    .into(),
                );
            }

            Ok(())
        }

        Component::CurDir
        | Component::RootDir
        | Component::Prefix(_) => Ok(()),
    }
}

/// Returns true if character is allowed in key segments.
#[must_use]
const fn is_segment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

/// Returns true if character is allowed in project names.
#[must_use]
const fn is_project_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}