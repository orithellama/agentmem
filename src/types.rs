use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::validation;
use crate::error::{Result, ValidationError};

/// A validated key used to address values in the store.
///
/// Keys are UTF-8 strings with a constrained structure intended to remain:
///
/// - human-readable
/// - namespace-friendly
/// - safe to persist
/// - predictable for agents and tooling
///
/// Examples:
///
/// - `agent/claude/current_task`
/// - `project/demo/root`
/// - `session/2026-04-12/summary`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(String);

impl Key {
    /// Validates and constructs a new key.
    ///
    /// The input must satisfy the crate's key rules as defined in the
    /// validation module.
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validation::validate_key(&input)?;
        Ok(Self(input))
    }

    /// Constructs a key without validation.
    ///
    /// This is intentionally crate-visible only. External callers should always
    /// go through [`Key::new`] so invariants remain enforced at the boundary.
    pub(crate) fn new_unchecked(input: String) -> Self {
        Self(input)
    }

    /// Returns the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the namespace portion of the key, if present.
    ///
    /// For a key like `agent/claude/current_task`, the namespace is
    /// `agent/claude`.
    #[must_use]
    pub fn namespace(&self) -> Option<Namespace> {
        self.0
            .rsplit_once('/')
            .and_then(|(prefix, _)| Namespace::new(prefix).ok())
    }

    /// Returns the final segment of the key.
    ///
    /// For a key like `agent/claude/current_task`, the leaf is `current_task`.
    #[must_use]
    pub fn leaf(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(self.0.as_str())
    }

    /// Returns `true` if the key is within the provided namespace.
    #[must_use]
    pub fn starts_with_namespace(&self, namespace: &Namespace) -> bool {
        if self.0 == namespace.as_str() {
            return true;
        }

        self.0
            .strip_prefix(namespace.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
    }

    /// Consumes the key and returns the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Key {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Key {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Key {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

/// A validated namespace prefix used to group related keys.
///
/// Namespaces are structured path-like identifiers without trailing slashes.
/// They are intended for agent scoping and project organization.
///
/// Examples:
///
/// - `agent/claude`
/// - `project/demo`
/// - `session/2026-04-12`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Namespace(String);

impl Namespace {
    /// Validates and constructs a new namespace.
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validation::validate_namespace(&input)?;
        Ok(Self(input))
    }

    /// Constructs a namespace without validation.
    pub(crate) fn new_unchecked(input: String) -> Self {
        Self(input)
    }

    /// Returns the namespace as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a new key under this namespace.
    ///
    /// The leaf segment is validated before constructing the final key.
    pub fn join(&self, leaf: &str) -> Result<Key> {
        validation::validate_key_leaf(leaf)?;
        Key::new(format!("{}/{}", self.0, leaf))
    }

    /// Returns the parent namespace, if any.
    ///
    /// For `agent/claude`, the parent is `agent`.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once('/')
            .map(|(parent, _)| Self::new_unchecked(parent.to_owned()))
    }

    /// Returns the final namespace segment.
    #[must_use]
    pub fn leaf(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(self.0.as_str())
    }

    /// Consumes the namespace and returns the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Namespace {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Namespace {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Namespace {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Namespace {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Namespace {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

/// A validated value stored in the memory engine.
///
/// Values remain text in v1 on purpose. This keeps the initial storage model:
///
/// - inspectable
/// - easy to serialize safely
/// - stable for CLI and agent use
///
/// The wrapper exists so length limits and future policy checks remain
/// centralized rather than spread across the codebase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Value(String);

impl Value {
    /// Validates and constructs a new value.
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validation::validate_value(&input)?;
        Ok(Self(input))
    }

    /// Constructs a value without validation.
    pub(crate) fn new_unchecked(input: String) -> Self {
        Self(input)
    }

    /// Returns the value as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns `true` when the value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the UTF-8 byte length of the value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl AsRef<str> for Value {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Value {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Value {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Value {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Value {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

/// A validated project name used during onboarding and configuration.
///
/// This is intentionally stricter than a free-form display label because it may
/// be used in:
///
/// - default namespaces
/// - config values
/// - file and directory suggestions
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectName(String);

impl ProjectName {
    /// Validates and constructs a new project name.
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validation::validate_project_name(&input)?;
        Ok(Self(input))
    }

    /// Constructs a project name without validation.
    pub(crate) fn new_unchecked(input: String) -> Self {
        Self(input)
    }

    /// Returns the project name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a default namespace rooted under this project.
    ///
    /// Example:
    ///
    /// `my-app` -> `project/my-app`
    pub fn as_project_namespace(&self) -> Namespace {
        Namespace::new_unchecked(format!("project/{}", self.0))
    }

    /// Consumes the project name and returns the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for ProjectName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ProjectName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for ProjectName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ProjectName {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ProjectName {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

/// A validated filesystem path pointing to a store file.
///
/// This wrapper exists to prevent raw `PathBuf` values from floating through
/// the API without basic policy checks.
///
/// Important:
///
/// Validation here is *policy validation*, not a security proof. Paths remain
/// environment-dependent and must still be handled carefully by the storage
/// layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorePath(PathBuf);

impl StorePath {
    /// Validates and constructs a new store path.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        validation::validate_store_path(&path)?;
        Ok(Self(path))
    }

    /// Constructs a store path without validation.
    pub(crate) fn new_unchecked(path: PathBuf) -> Self {
        Self(path)
    }

    /// Returns the path as a borrowed [`Path`].
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns the parent directory, if one exists.
    #[must_use]
    pub fn parent(&self) -> Option<&Path> {
        self.0.parent()
    }

    /// Returns the file name component, if one exists.
    #[must_use]
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.0.file_name()
    }

    /// Consumes the wrapper and returns the inner path buffer.
    #[must_use]
    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

impl AsRef<Path> for StorePath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Borrow<Path> for StorePath {
    fn borrow(&self) -> &Path {
        self.as_path()
    }
}

impl Deref for StorePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl fmt::Display for StorePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_path_for_display(&self.0, f)
    }
}

impl TryFrom<PathBuf> for StorePath {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: PathBuf) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&Path> for StorePath {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &Path) -> Result<Self> {
        Self::new(value.to_path_buf())
    }
}

impl TryFrom<&str> for StorePath {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(PathBuf::from(value))
    }
}

/// Writes a path in a user-facing way without panicking on non-UTF-8 content.
fn write_path_for_display(path: &Path, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", path.display())
}

/// A typed key/value entry returned by store iteration and listing APIs.
///
/// This is preferable to returning a tuple in public-facing code because the
/// field names remain self-documenting and easy to evolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The validated key for this entry.
    pub key: Key,
    /// The validated value for this entry.
    pub value: Value,
}

impl Entry {
    /// Constructs a new entry from validated key and value types.
    #[must_use]
    pub const fn new(key: Key, value: Value) -> Self {
        Self { key, value }
    }

    /// Creates an entry from raw strings after validation.
    pub fn try_new(key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        Ok(Self {
            key: Key::new(key.into())?,
            value: Value::new(value.into())?,
        })
    }
}

/// A typed request payload for inserting or replacing an entry.
///
/// This can help keep higher-level APIs explicit when we later want methods
/// like `Store::insert(entry)` alongside `Store::set(key, value)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRequest {
    /// Target key.
    pub key: Key,
    /// Value to persist.
    pub value: Value,
}

impl SetRequest {
    /// Constructs a new request from validated parts.
    #[must_use]
    pub const fn new(key: Key, value: Value) -> Self {
        Self { key, value }
    }

    /// Constructs a request from raw strings after validation.
    pub fn try_new(key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        Ok(Self {
            key: Key::new(key.into())?,
            value: Value::new(value.into())?,
        })
    }
}

/// A small typed wrapper used for prefix-based list operations.
///
/// This is intentionally separate from [`Namespace`] so the API can later allow:
///
/// - exact namespaces
/// - looser prefixes
/// - migration filters
///
/// without overloading one type with too many meanings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyPrefix(String);

impl KeyPrefix {
    /// Validates and constructs a prefix.
    ///
    /// Prefixes use the same structural rules as namespaces in v1.
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validation::validate_namespace(&input)?;
        Ok(Self(input))
    }

    /// Returns the prefix as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns `true` when the provided key matches this prefix.
    #[must_use]
    pub fn matches(&self, key: &Key) -> bool {
        if key.as_str() == self.0 {
            return true;
        }

        key.as_str()
            .strip_prefix(self.0.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

impl fmt::Display for KeyPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for KeyPrefix {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<String> for KeyPrefix {
    type Error = crate::error::AgentMemoryError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

/// Ensures a path-like field is never constructed from an empty or malformed
/// user-facing string without validation.
///
/// This helper is kept private because callers should generally use the typed
/// constructors above instead of handling raw validation details here.
fn _ensure_non_empty(field: &'static str, value: &str) -> std::result::Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::empty(field));
    }

    Ok(())
}