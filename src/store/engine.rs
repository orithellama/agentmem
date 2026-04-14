use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::core::map::MemoryMap;
use crate::error::{Result, StoreError};
use crate::store::locking::StoreLock;
use crate::store::migration::STORE_FORMAT_VERSION;
use crate::store::persist;
use crate::types::{Entry, Key, KeyPrefix, Namespace, ProjectName, SetRequest, StorePath, Value};

/// Primary durable memory store.
///
/// `Store` owns:
///
/// - validated runtime configuration
/// - in-memory state
/// - optional lock guard protecting write access
///
/// This is the main type external consumers should use.
#[derive(Debug)]
pub struct Store {
    config: Config,
    map: MemoryMap,
    lock: Option<StoreLock>,
}

/// Human-readable metadata about an opened store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreInfo {
    /// Store format version.
    pub format_version: u32,
    /// Config version.
    pub config_version: u32,
    /// Project name attached to this store.
    pub project_name: ProjectName,
    /// Backing file path.
    pub path: StorePath,
}

/// Lightweight runtime statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreStats {
    /// Number of entries.
    pub entry_count: usize,
    /// Whether the store contains zero entries.
    pub is_empty: bool,
    /// Whether a write lock is currently held.
    pub locked: bool,
}

impl Store {
    /// Creates a new empty store using the provided validated config.
    ///
    /// This does not write to disk until `flush()` is called.
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            map: MemoryMap::new(),
            lock: None,
        })
    }

    /// Opens an existing store or creates a new in-memory empty one if the file
    /// does not yet exist.
    ///
    /// No lock is acquired automatically.
    pub fn open(config: Config) -> Result<Self> {
        config.validate()?;

        let path = config.store_path().as_path();

        if path.exists() {
            let map = persist::load_map(path)?;
            Ok(Self {
                config,
                map,
                lock: None,
            })
        } else {
            Self::new(config)
        }
    }

    /// Opens a store and acquires an exclusive write lock.
    pub fn open_locked(config: Config) -> Result<Self> {
        let mut store = Self::open(config)?;
        store.acquire_lock()?;
        Ok(store)
    }

    /// Creates a conventional project-local store rooted at `project_root`.
    pub fn open_project(project_name: ProjectName, project_root: impl AsRef<Path>) -> Result<Self> {
        let config = Config::for_project_root(project_name, project_root)?;
        Self::open(config)
    }

    /// Returns immutable config access.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Returns store metadata.
    #[must_use]
    pub fn info(&self) -> StoreInfo {
        StoreInfo {
            format_version: STORE_FORMAT_VERSION,
            config_version: self.config.version(),
            project_name: self.config.project_name().clone(),
            path: self.config.store_path().clone(),
        }
    }

    /// Returns runtime statistics.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        StoreStats {
            entry_count: self.map.len(),
            is_empty: self.map.is_empty(),
            locked: self.lock.is_some(),
        }
    }

    /// Returns number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the store has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns true if a key exists.
    #[must_use]
    pub fn contains(&self, key: &Key) -> bool {
        self.map.contains_key(key)
    }

    /// Gets a value by key.
    #[must_use]
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    /// Gets a value or returns a not-found error.
    pub fn require(&self, key: &Key) -> Result<&Value> {
        self.map.require(key)
    }

    /// Inserts or replaces a key/value pair.
    ///
    /// This mutates only in memory until `flush()` is called.
    pub fn set(&mut self, key: Key, value: Value) -> Result<Option<Value>> {
        self.map.insert(key, value)
    }

    /// Inserts or replaces using a typed request.
    pub fn apply(&mut self, request: SetRequest) -> Result<Option<Value>> {
        self.map.set(request)
    }

    /// Removes a key if present.
    pub fn delete(&mut self, key: &Key) -> Option<Value> {
        self.map.remove(key)
    }

    /// Removes a key or returns an error.
    pub fn delete_required(&mut self, key: &Key) -> Result<Value> {
        self.map.remove_required(key)
    }

    /// Removes all entries.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns all entries in sorted deterministic order.
    #[must_use]
    pub fn entries(&self) -> Vec<Entry> {
        self.map.entries()
    }

    /// Lists entries under a namespace.
    #[must_use]
    pub fn list_namespace(&self, namespace: &Namespace) -> Vec<Entry> {
        let prefix = KeyPrefix::new(namespace.as_str())
            .expect("validated namespace must always form a valid prefix");

        self.map.entries_with_prefix(&prefix)
    }

    /// Lists entries under an explicit prefix.
    #[must_use]
    pub fn list_prefix(&self, prefix: &KeyPrefix) -> Vec<Entry> {
        self.map.entries_with_prefix(prefix)
    }

    /// Removes all entries under a namespace.
    ///
    /// Returns number of removed entries.
    pub fn delete_namespace(&mut self, namespace: &Namespace) -> usize {
        let prefix = KeyPrefix::new(namespace.as_str())
            .expect("validated namespace must always form a valid prefix");

        self.map.remove_prefix(&prefix)
    }

    /// Ensures parent directories exist for the store path.
    pub fn prepare_path(&self) -> Result<()> {
        let path = self.config.store_path().as_path();

        let parent = path
            .parent()
            .ok_or_else(|| StoreError::malformed("store path must contain a parent directory"))?;

        fs::create_dir_all(parent).map_err(|source| {
            StoreError::PreparePath {
                path: parent.to_path_buf(),
                source,
            }
            .into()
        })
    }

    /// Flushes the current in-memory state to disk using atomic persistence.
    ///
    /// If a lock exists, it is assumed to protect this write path.
    pub fn flush(&self) -> Result<()> {
        if self.lock.is_none() {
            return Err(StoreError::malformed("flush requires acquired store lock").into());
        }

        self.prepare_path()?;

        let path = self.config.store_path().as_path();
        persist::save_map(path, &self.map)
    }

    /// Reloads state from disk, replacing in-memory contents.
    pub fn reload(&mut self) -> Result<()> {
        let path = self.config.store_path().as_path();

        if !path.exists() {
            self.map.clear();
            return Ok(());
        }

        self.map = persist::load_map(path)?;
        Ok(())
    }

    /// Acquires an exclusive lock if not already held.
    pub fn acquire_lock(&mut self) -> Result<()> {
        if self.lock.is_some() {
            return Ok(());
        }

        let guard = StoreLock::acquire(self.config.store_path().as_path())?;
        self.lock = Some(guard);

        Ok(())
    }

    /// Releases the lock if held.
    pub fn release_lock(&mut self) -> Result<()> {
        if let Some(lock) = self.lock.take() {
            lock.release()?;
        }

        Ok(())
    }

    /// Returns true if the store currently holds a lock.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    /// Consumes the store and returns the inner config.
    #[must_use]
    pub fn into_config(self) -> Config {
        self.config.clone()
    }

    /// Returns the backing path.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.config.store_path().as_path()
    }

    /// Returns a cloned owned path.
    #[must_use]
    pub fn path_buf(&self) -> PathBuf {
        self.path().to_path_buf()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(lock) = self.lock.take() {
            let _ = lock.release();
        }
    }
}
