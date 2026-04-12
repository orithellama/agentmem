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
    within_range, MAX_KEY_LEN, MAX_KEY_SEGMENT_LEN, MAX_NAMESPACE_LEN, MAX_PROJECT_NAME_LEN,
    MAX_SEGMENT_COUNT, MAX_STORE_PATH_LEN, MAX_VALUE_LEN, MIN_KEY_LEN, MIN_KEY_SEGMENT_LEN,
    MIN_NAMESPACE_LEN, MIN_PROJECT_NAME_LEN, MIN_VALUE_LEN,
};
use crate::error::{Result, ValidationError};

/// Validates a fully qualified key.
///
/// Expected examples:
///
/// - `agent/claude/current_task`
/// - `project/demo/root`
/// - `session/2026-04-12/state`
pub fn validate_key(input: &str) -> Result<()> {
    validate_common_text("key", input, MIN_KEY_LEN, MAX_KEY_LEN)?;

    if input.starts_with('/') || input.ends_with('/') {
        return Err(ValidationError::invalid_format(
            "key",
            "must not start or end with '/'",
        )
        .into());
    }

    if input.contains("//") {
        return Err(ValidationError::invalid_format(
            "key",
            "must not contain empty path segments",
        )
        .into());
    }

    let segments: Vec<&str> = input.split('/').collect();

    if segments.len() > MAX_SEGMENT_COUNT {
        return Err(ValidationError::too_long(
            "key_segments",
            segments.len(),
            MAX_SEGMENT_COUNT,
        )
        .into());
    }

    for segment in segments {
        validate_segment("key", segment)?;
    }

    Ok(())
}

/// Validates a namespace prefix.
///
/// Expected examples:
///
/// - `agent/claude`
/// - `project/demo`
/// - `session/2026-04-12`
pub fn validate_namespace(input: &str) -> Result<()> {
    validate_common_text(
        "namespace",
        input,
        MIN_NAMESPACE_LEN,
        MAX_NAMESPACE_LEN,
    )?;

    if input.starts_with('/') || input.ends_with('/') {
        return Err(ValidationError::invalid_format(
            "namespace",
            "must not start or end with '/'",
        )
        .into());
    }

    if input.contains("//") {
        return Err(ValidationError::invalid_format(
            "namespace",
            "must not contain empty path segments",
        )
        .into());
    }

    let segments: Vec<&str> = input.split('/').collect();

    if segments.len() > MAX_SEGMENT_COUNT {
        return Err(ValidationError::too_long(
            "namespace_segments",
            segments.len(),
            MAX_SEGMENT_COUNT,
        )
        .into());
    }

    for segment in segments {
        validate_segment("namespace", segment)?;
    }

    Ok(())
}

/// Validates a leaf segment intended to be appended to a namespace.
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
/// Values remain text in v1, but size boundaries are enforced.
pub fn validate_value(input: &str) -> Result<()> {
    validate_common_text("value", input, MIN_VALUE_LEN, MAX_VALUE_LEN)?;

    if input.contains('\0') {
        return Err(ValidationError::invalid_format(
            "value",
            "must not contain NUL bytes",
        )
        .into());
    }

    Ok(())
}

/// Validates a project name.
///
/// Project names are stricter than generic values because they may be used in:
///
/// - default namespaces
/// - config fields
/// - suggested directories
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

        return Err(ValidationError::InvalidCharacter {
            field: "project_name",
            character: ch,
            index,
        }
        .into());
    }

    if input.starts_with('-') || input.ends_with('-') {
        return Err(ValidationError::invalid_format(
            "project_name",
            "must not start or end with '-'",
        )
        .into());
    }

    Ok(())
}

/// Validates a store file path.
///
/// This is policy validation, not a security guarantee.
pub fn validate_store_path(path: &Path) -> Result<()> {
    let rendered = path.to_string_lossy();

    if rendered.is_empty() {
        return Err(ValidationError::empty("store_path").into());
    }

    if rendered.len() > MAX_STORE_PATH_LEN {
        return Err(
            ValidationError::too_long("store_path", rendered.len(), MAX_STORE_PATH_LEN).into(),
        );
    }

    let file_name = path.file_name().ok_or_else(|| {
        ValidationError::invalid_path("store_path", "path must include a file name")
    })?;

    if file_name.to_string_lossy().trim().is_empty() {
        return Err(ValidationError::invalid_path(
            "store_path",
            "file name must not be empty",
        )
        .into());
    }

    for component in path.components() {
        validate_path_component(component)?;
    }

    Ok(())
}

/// Validates a single namespace/key segment.
///
/// Allowed characters:
///
/// - `a-z`
/// - `A-Z`
/// - `0-9`
/// - `_`
/// - `-`
/// - `.`
fn validate_segment(field: &'static str, segment: &str) -> Result<()> {
    validate_common_text(
        field,
        segment,
        MIN_KEY_SEGMENT_LEN,
        MAX_KEY_SEGMENT_LEN,
    )?;

    if segment == "." || segment == ".." {
        return Err(ValidationError::invalid_segment(
            field,
            "reserved segment is not allowed",
        )
        .into());
    }

    for (index, ch) in segment.char_indices() {
        if is_segment_char(ch) {
            continue;
        }

        return Err(ValidationError::InvalidCharacter {
            field,
            character: ch,
            index,
        }
        .into());
    }

    Ok(())
}

/// Shared validation for bounded UTF-8 text fields.
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
            return Err(ValidationError::too_short(field, len, min_len).into());
        }

        return Err(ValidationError::too_long(field, len, max_len).into());
    }

    if input.contains('\0') {
        return Err(ValidationError::invalid_format(
            field,
            "must not contain NUL bytes",
        )
        .into());
    }

    Ok(())
}

/// Validates path components for obvious misuse patterns.
///
/// Notes:
/// - Relative paths are allowed.
/// - Parent traversal segments are rejected.
/// - Prefix/root components are allowed where the platform emits them.
fn validate_path_component(component: Component<'_>) -> Result<()> {
    match component {
        Component::ParentDir => Err(ValidationError::invalid_path(
            "store_path",
            "parent traversal ('..') is not allowed",
        )
        .into()),

        Component::Normal(name) => {
            let rendered = name.to_string_lossy();

            if rendered.trim().is_empty() {
                return Err(ValidationError::invalid_path(
                    "store_path",
                    "path segment must not be empty",
                )
                .into());
            }

            if rendered == "." || rendered == ".." {
                return Err(ValidationError::invalid_path(
                    "store_path",
                    "reserved path segment is not allowed",
                )
                .into());
            }

            Ok(())
        }

        Component::CurDir | Component::RootDir | Component::Prefix(_) => Ok(()),
    }
}

/// Returns `true` if a character is allowed in key/namespace segments.
#[must_use]
const fn is_segment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

/// Returns `true` if a character is allowed in project names.
#[must_use]
const fn is_project_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}