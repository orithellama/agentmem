//! Cross-process store locking.
//!
//! The store should protect write operations from concurrent mutation by
//! multiple processes. This module provides a simple lock-file strategy that is:
//!
//! - explicit
//! - portable
//! - dependency-light
//! - understandable by operators
//!
//! Current model:
//!
//! - acquire lock by atomically creating a sibling `.lock` file
//! - lock file contains metadata for debugging
//! - releasing removes the lock file
//!
//! This is intentionally conservative for v1.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{LockError, Result};

/// Default extension used for lock files.
///
/// Example:
///
/// `store.json` -> `store.json.lock`
pub const LOCK_EXTENSION: &str = "lock";

/// Exclusive lock guard.
///
/// When dropped normally, callers should still explicitly call `release()` so
/// failures can be handled rather than silently ignored.
#[derive(Debug)]
pub struct StoreLock {
    path: PathBuf,
    released: bool,
}

impl StoreLock {
    /// Acquires an exclusive lock for the given store path.
    ///
    /// This uses `create_new(true)` which maps to an atomic create-if-missing
    /// behavior on supported platforms.
    pub fn acquire(store_path: &Path) -> Result<Self> {
        let lock_path = lock_path_for(store_path);

        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|source| LockError::Acquire {
                path: lock_path.clone(),
                source,
            })?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    LockError::AlreadyHeld {
                        path: lock_path.clone(),
                    }
                } else {
                    LockError::Acquire {
                        path: lock_path.clone(),
                        source,
                    }
                }
            })?;

        let metadata = LockMetadata::current();

        let payload = metadata.render();

        file.write_all(payload.as_bytes())
            .map_err(|source| LockError::Acquire {
                path: lock_path.clone(),
                source,
            })?;

        file.flush().map_err(|source| LockError::Acquire {
            path: lock_path.clone(),
            source,
        })?;

        Ok(Self {
            path: lock_path,
            released: false,
        })
    }

    /// Releases the lock.
    ///
    /// Safe to call once. Subsequent calls are no-ops.
    pub fn release(mut self) -> Result<()> {
        self.release_inner()
    }

    /// Returns the lock file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn release_inner(&mut self) -> Result<()> {
        if self.released {
            return Ok(());
        }

        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|source| LockError::Release {
                path: self.path.clone(),
                source,
            })?;
        }

        self.released = true;
        Ok(())
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.release_inner();
    }
}

/// Returns the canonical lock file path for a store file.
///
/// Example:
///
/// `store.json` -> `store.json.lock`
#[must_use]
pub fn lock_path_for(store_path: &Path) -> PathBuf {
    let rendered = format!("{}.{}", store_path.to_string_lossy(), LOCK_EXTENSION);

    PathBuf::from(rendered)
}

/// Returns true if a lock currently exists for the store path.
#[must_use]
pub fn is_locked(store_path: &Path) -> bool {
    lock_path_for(store_path).exists()
}

/// Best-effort removal of a stale lock.
///
/// Use with caution. Prefer explicit lock ownership flows.
pub fn remove_stale_lock(store_path: &Path) -> Result<()> {
    let path = lock_path_for(store_path);

    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(&path).map_err(|source| LockError::Release { path, source }.into())
}

/// Human-readable metadata written into lock files.
///
/// This is intentionally plain text so operators can inspect it quickly.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LockMetadata {
    pid: u32,
    created_unix_seconds: u64,
}

impl LockMetadata {
    fn current() -> Self {
        Self {
            pid: process::id(),
            created_unix_seconds: unix_now(),
        }
    }

    fn render(&self) -> String {
        format!(
            "pid={}\ncreated_unix_seconds={}\n",
            self.pid, self.created_unix_seconds
        )
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
