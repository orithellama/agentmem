//! Write-ahead journal support.
//!
//! V1 persistence works through full atomic snapshots. This module introduces a
//! structured journal layer for future-safe recovery, diagnostics and optional
//! lower-latency write patterns.
//!
//! Current posture:
//!
//! - safe to include now
//! - usable for append-only operation logs
//! - optional for callers
//! - does not replace canonical snapshot persistence yet
//!
//! Typical future flow:
//!
//! 1. append journal operation
//! 2. fsync journal
//! 3. apply in-memory mutation
//! 4. periodic snapshot + truncate journal

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::types::{Key, Value};

/// Current journal schema version.
pub const JOURNAL_VERSION: u32 = 1;

/// Default journal file name.
pub const DEFAULT_JOURNAL_FILE_NAME: &str = "store.journal";

/// Single append-only operation written to the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JournalOperation {
    /// Insert or replace a key/value pair.
    Set {
        /// Validated key.
        key: String,
        /// Value payload.
        value: String,
    },

    /// Delete a single key.
    Delete {
        /// Validated key.
        key: String,
    },

    /// Remove all state.
    Clear,
}

/// Journal entry wrapper containing versioned metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Schema version.
    pub version: u32,

    /// Monotonic sequence number supplied by caller.
    pub sequence: u64,

    /// Operation payload.
    pub operation: JournalOperation,
}

/// Append-only journal writer.
///
/// Holds the file path only; opens lazily on append so short-lived commands
/// do not keep handles around unnecessarily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal {
    path: PathBuf,
}

impl Journal {
    /// Creates a journal handle for a specific file path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Creates a conventional sibling journal path next to a store file.
    ///
    /// Example:
    ///
    /// `/repo/.agentmem/store.json`
    ///
    /// becomes:
    ///
    /// `/repo/.agentmem/store.journal`
    #[must_use]
    pub fn alongside_store(store_path: &Path) -> Self {
        let path = store_path.with_file_name(DEFAULT_JOURNAL_FILE_NAME);
        Self::new(path)
    }

    /// Returns the journal path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true if the journal file currently exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// Appends a typed entry as a single JSON line.
    pub fn append(&self, entry: &JournalEntry) -> Result<()> {
        validate_entry(entry)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StoreError::PreparePath {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| StoreError::Write {
                path: self.path.clone(),
                source,
            })?;

        let mut writer = BufWriter::new(file);

        let line = serde_json::to_string(entry).map_err(|error| StoreError::Serialize {
            message: format!("failed to serialize journal entry: {error}"),
        })?;

        writer
            .write_all(line.as_bytes())
            .map_err(|source| StoreError::Write {
                path: self.path.clone(),
                source,
            })?;

        writer
            .write_all(b"\n")
            .map_err(|source| StoreError::Write {
                path: self.path.clone(),
                source,
            })?;

        writer.flush().map_err(|source| StoreError::Write {
            path: self.path.clone(),
            source,
        })?;

        Ok(())
    }

    /// Reads and parses all valid journal entries in order.
    pub fn read_all(&self) -> Result<Vec<JournalEntry>> {
        if !self.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path).map_err(|source| StoreError::Read {
            path: self.path.clone(),
            source,
        })?;

        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| StoreError::Read {
                path: self.path.clone(),
                source,
            })?;

            if line.trim().is_empty() {
                continue;
            }

            let entry: JournalEntry =
                serde_json::from_str(&line).map_err(|error| StoreError::Malformed {
                    reason: format!("invalid journal line {}: {}", line_number + 1, error),
                })?;

            validate_entry(&entry)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Truncates the journal to zero length if it exists.
    pub fn clear(&self) -> Result<()> {
        if !self.exists() {
            return Ok(());
        }

        File::create(&self.path).map_err(|source| StoreError::Write {
            path: self.path.clone(),
            source,
        })?;

        Ok(())
    }

    /// Removes the journal file entirely.
    pub fn remove(&self) -> Result<()> {
        if !self.exists() {
            return Ok(());
        }

        fs::remove_file(&self.path).map_err(|source| StoreError::Write {
            path: self.path.clone(),
            source,
        })?;

        Ok(())
    }
}

impl JournalEntry {
    /// Constructs a `set` operation.
    pub fn set(sequence: u64, key: &Key, value: &Value) -> Self {
        Self {
            version: JOURNAL_VERSION,
            sequence,
            operation: JournalOperation::Set {
                key: key.as_str().to_owned(),
                value: value.as_str().to_owned(),
            },
        }
    }

    /// Constructs a `delete` operation.
    pub fn delete(sequence: u64, key: &Key) -> Self {
        Self {
            version: JOURNAL_VERSION,
            sequence,
            operation: JournalOperation::Delete {
                key: key.as_str().to_owned(),
            },
        }
    }

    /// Constructs a `clear` operation.
    #[must_use]
    pub const fn clear(sequence: u64) -> Self {
        Self {
            version: JOURNAL_VERSION,
            sequence,
            operation: JournalOperation::Clear,
        }
    }
}

/// Validates journal entry structure.
fn validate_entry(entry: &JournalEntry) -> Result<()> {
    if entry.version != JOURNAL_VERSION {
        return Err(StoreError::Malformed {
            reason: format!("unsupported journal version: {}", entry.version),
        }
        .into());
    }

    match &entry.operation {
        JournalOperation::Set { key, value } => {
            let _ = Key::new(key.clone())?;
            let _ = Value::new(value.clone())?;
        }

        JournalOperation::Delete { key } => {
            let _ = Key::new(key.clone())?;
        }

        JournalOperation::Clear => {}
    }

    Ok(())
}
