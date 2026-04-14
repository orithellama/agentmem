use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::limits::MAX_STORE_FILE_NAME_LEN;
use crate::core::map::MemoryMap;
use crate::error::{Result, StoreError};
use crate::store::migration::STORE_FORMAT_VERSION;
use crate::types::{Entry, Key, Value};

/// Serializable on-disk store container.
///
/// The storage format is intentionally explicit:
///
/// - schema version
/// - ordered records
///
/// This makes migrations and validation easier than serializing a raw map
/// directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreFile {
    /// File format version.
    pub version: u32,

    /// Ordered records.
    pub records: Vec<StoreRecord>,
}

/// Serializable key/value record.
///
/// Keys and values remain strings in v1 for inspectability and operational
/// simplicity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRecord {
    /// Fully-qualified validated key.
    pub key: String,

    /// Stored textual value.
    pub value: String,
}

/// Loads a map from disk.
///
/// If the file does not exist, callers should decide whether that means:
///
/// - create empty store
/// - surface an error
///
/// This function expects the file to exist.
pub fn load_map(path: &Path) -> Result<MemoryMap> {
    let raw = read_store_file(path)?;
    let parsed = parse_store_file(&raw)?;
    file_to_map(parsed)
}

/// Saves a map to disk using atomic persistence.
///
/// Writes to a temporary sibling file first, then renames.
pub fn save_map(path: &Path, map: &MemoryMap) -> Result<()> {
    let file = map_to_file(map);
    let payload = serialize_store_file(&file)?;

    write_atomic(path, payload.as_bytes())
}

/// Parses a JSON store payload into a typed store file.
pub fn parse_store_file(input: &str) -> Result<StoreFile> {
    let parsed: StoreFile =
        serde_json::from_str(input).map_err(|error| StoreError::Deserialize {
            message: format!("invalid store JSON: {error}"),
        })?;

    validate_store_file(&parsed)?;
    Ok(parsed)
}

/// Serializes a store file as compact JSON.
///
/// Compact encoding significantly reduces disk usage for large indexes.
pub fn serialize_store_file(file: &StoreFile) -> Result<String> {
    validate_store_file(file)?;

    serde_json::to_string(file).map_err(|error| {
        StoreError::Serialize {
            message: format!("failed to serialize store: {error}"),
        }
        .into()
    })
}

/// Converts an in-memory map into the stable file representation.
#[must_use]
pub fn map_to_file(map: &MemoryMap) -> StoreFile {
    let records = map
        .entries()
        .into_iter()
        .map(|entry| StoreRecord {
            key: entry.key.into_inner(),
            value: entry.value.into_inner(),
        })
        .collect();

    StoreFile {
        version: STORE_FORMAT_VERSION,
        records,
    }
}

/// Converts a parsed file representation into an in-memory map.
pub fn file_to_map(file: StoreFile) -> Result<MemoryMap> {
    validate_store_file(&file)?;

    let mut entries = Vec::with_capacity(file.records.len());

    for record in file.records {
        let entry = Entry::try_new(record.key, record.value)?;
        entries.push(entry);
    }

    Ok(entries.into_iter().collect())
}

/// Validates a parsed store file before use.
pub fn validate_store_file(file: &StoreFile) -> Result<()> {
    if file.version != STORE_FORMAT_VERSION {
        return Err(StoreError::UnsupportedVersion {
            version: file.version,
        }
        .into());
    }

    for record in &file.records {
        let _ = Key::new(record.key.clone())?;
        let _ = Value::new(record.value.clone())?;
    }

    Ok(())
}

/// Reads a store file into memory with size checks.
fn read_store_file(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path).map_err(|source| StoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let size = usize::try_from(metadata.len()).map_err(|_| StoreError::Malformed {
        reason: "file size exceeds supported platform limits".to_owned(),
    })?;

    if size > MAX_STORE_FILE_NAME_LEN {
        return Err(StoreError::Malformed {
            reason: format!(
                "store file too large: {} bytes exceeds max {}",
                size, MAX_STORE_FILE_NAME_LEN
            ),
        }
        .into());
    }

    fs::read_to_string(path).map_err(|source| {
        StoreError::Read {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

/// Writes a payload atomically.
///
/// Strategy:
///
/// 1. write sibling temp file
/// 2. rename over target
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| StoreError::malformed("target path must contain a parent directory"))?;

    fs::create_dir_all(parent).map_err(|source| StoreError::PreparePath {
        path: parent.to_path_buf(),
        source,
    })?;

    let file_name = target
        .file_name()
        .ok_or_else(|| StoreError::malformed("target path must contain a file name"))?;

    let temp_path = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));

    fs::write(&temp_path, bytes).map_err(|source| StoreError::Write {
        path: temp_path.clone(),
        source,
    })?;

    fs::rename(&temp_path, target).map_err(|source| StoreError::AtomicPersist {
        path: target.to_path_buf(),
        reason: format!("rename failed: {source}"),
    })?;

    Ok(())
}

/// Creates an empty store file payload.
#[must_use]
pub fn empty_store_file() -> StoreFile {
    StoreFile {
        version: STORE_FORMAT_VERSION,
        records: Vec::new(),
    }
}

/// Creates an empty map and writes it if the target does not exist.
pub fn initialize_if_missing(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let file = empty_store_file();
    let payload = serialize_store_file(&file)?;

    write_atomic(path, payload.as_bytes())
}

/// Returns true if the given path appears to contain a readable store file.
#[must_use]
pub fn exists(path: &Path) -> bool {
    path.is_file()
}

/// Best-effort removal of a store file.
pub fn remove(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(path).map_err(|source| {
        StoreError::Write {
            path: PathBuf::from(path),
            source: io::Error::new(source.kind(), source.to_string()),
        }
        .into()
    })
}