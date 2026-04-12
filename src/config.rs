use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{AgentMemoryError, ConfigError, Result, ValidationError};
use crate::types::{ProjectName, StorePath};

/// Current on-disk configuration format version.
///
/// This must be incremented for any incompatible config change.
pub const CONFIG_VERSION: u32 = 1;

/// Default file name used for project-local configuration.
pub const DEFAULT_CONFIG_FILE_NAME: &str = "agentmem.json";

/// Default hidden directory used for project-local state.
pub const DEFAULT_STATE_DIR_NAME: &str = ".agentmem";

/// Default store file name created inside the state directory.
pub const DEFAULT_STORE_FILE_NAME: &str = "store.json";

/// High-level application configuration.
///
/// This type represents validated, runtime-safe configuration used by the
/// library and CLI. It deliberately avoids exposing raw, unchecked strings for
/// fields that matter to correctness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Version of the configuration schema.
    version: u32,
    /// Human-readable but validated project identifier.
    project_name: ProjectName,
    /// Validated path to the store file.
    store_path: StorePath,
}

impl Config {
    /// Creates a new validated configuration.
    pub fn new(project_name: ProjectName, store_path: StorePath) -> Result<Self> {
        Self::validate_version(CONFIG_VERSION)?;

        Ok(Self {
            version: CONFIG_VERSION,
            project_name,
            store_path,
        })
    }

    /// Returns the configuration version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the validated project name.
    #[must_use]
    pub const fn project_name(&self) -> &ProjectName {
        &self.project_name
    }

    /// Returns the validated store path.
    #[must_use]
    pub const fn store_path(&self) -> &StorePath {
        &self.store_path
    }

    /// Returns the directory that contains the store file, if present.
    #[must_use]
    pub fn store_dir(&self) -> Option<&Path> {
        self.store_path.parent()
    }

    /// Converts the runtime config into its serializable representation.
    #[must_use]
    pub fn to_file(&self) -> ConfigFile {
        ConfigFile {
            version: self.version,
            project_name: self.project_name.as_str().to_owned(),
            store_path: self.store_path.as_path().to_path_buf(),
        }
    }

    /// Serializes the config as pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.to_file()).map_err(|error| {
            AgentMemoryError::Config(ConfigError::Parse {
                message: format!("failed to serialize configuration: {error}"),
            })
        })
    }

    /// Parses a configuration from a JSON string.
    pub fn from_json(input: &str) -> Result<Self> {
        let file: ConfigFile = serde_json::from_str(input).map_err(|error| {
            AgentMemoryError::Config(ConfigError::Parse {
                message: format!("failed to parse configuration JSON: {error}"),
            })
        })?;

        file.try_into()
    }

    /// Loads a config from a file on disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let raw = fs::read_to_string(path).map_err(|source| {
            AgentMemoryError::Io {
                source: io::Error::new(
                    source.kind(),
                    format!("failed to read config file {}: {source}", path.display()),
                ),
            }
        })?;

        Self::from_json(&raw)
    }

    /// Saves the config to disk using an atomic write strategy.
    ///
    /// The parent directory is created if missing.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AgentMemoryError::Io {
                source: io::Error::new(
                    source.kind(),
                    format!(
                        "failed to create config directory {}: {source}",
                        parent.display()
                    ),
                ),
            })?;
        }

        let payload = self.to_json_pretty()?;
        write_atomic(path, payload.as_bytes())
    }

    /// Builds a project-local config path inside the provided project root.
    ///
    /// Example:
    ///
    /// `/repo` -> `/repo/.agentmem/agentmem.json`
    pub fn project_config_path(project_root: impl AsRef<Path>) -> PathBuf {
        project_root
            .as_ref()
            .join(DEFAULT_STATE_DIR_NAME)
            .join(DEFAULT_CONFIG_FILE_NAME)
    }

    /// Builds a default project-local store path inside the provided project root.
    ///
    /// Example:
    ///
    /// `/repo` -> `/repo/.agentmem/store.json`
    pub fn project_store_path(project_root: impl AsRef<Path>) -> PathBuf {
        project_root
            .as_ref()
            .join(DEFAULT_STATE_DIR_NAME)
            .join(DEFAULT_STORE_FILE_NAME)
    }

    /// Returns a conventional user-level config path based on platform dirs.
    ///
    /// This does not create any directories. It only resolves the location.
    pub fn default_user_config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("com", "agent-memory", "agent-hashmap").ok_or_else(|| {
            AgentMemoryError::Config(ConfigError::Malformed {
                reason: "failed to resolve platform-specific config directory",
            })
        })?;

        Ok(dirs.config_dir().join(DEFAULT_CONFIG_FILE_NAME))
    }

    /// Returns a conventional user-level store path based on platform dirs.
    ///
    /// This does not create any directories. It only resolves the location.
    pub fn default_user_store_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("com", "agent-memory", "agent-hashmap").ok_or_else(|| {
            AgentMemoryError::Config(ConfigError::Malformed {
                reason: "failed to resolve platform-specific data directory",
            })
        })?;

        Ok(dirs.data_local_dir().join(DEFAULT_STORE_FILE_NAME))
    }

    /// Creates a validated config for a project rooted at `project_root`,
    /// using the conventional hidden `.agentmem` directory.
    pub fn for_project_root(
        project_name: ProjectName,
        project_root: impl AsRef<Path>,
    ) -> Result<Self> {
        let store_path = StorePath::new(Self::project_store_path(project_root))?;
        Self::new(project_name, store_path)
    }

    /// Ensures the config is internally coherent.
    pub fn validate(&self) -> Result<()> {
        Self::validate_version(self.version)?;

        if self.project_name.as_str().is_empty() {
            return Err(ValidationError::empty("project_name").into());
        }

        if self.store_path.file_name().is_none() {
            return Err(ValidationError::invalid_path(
                "store_path",
                "store path must include a file name",
            )
            .into());
        }

        Ok(())
    }

    fn validate_version(version: u32) -> Result<()> {
        if version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion { version }.into());
        }

        Ok(())
    }
}

/// Serializable on-disk representation of [`Config`].
///
/// This type is intentionally separate from the runtime config so the crate can:
///
/// - parse untrusted disk input first
/// - validate fields explicitly
/// - keep runtime invariants strong
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Schema version.
    pub version: u32,
    /// Project name as stored on disk.
    pub project_name: String,
    /// Path to the store file.
    pub store_path: PathBuf,
}

impl TryFrom<ConfigFile> for Config {
    type Error = AgentMemoryError;

    fn try_from(value: ConfigFile) -> Result<Self> {
        if value.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                version: value.version,
            }
            .into());
        }

        let project_name =
            ProjectName::new(value.project_name).map_err(|error| map_project_name_error(error))?;

        let store_path =
            StorePath::new(value.store_path).map_err(|error| map_store_path_error(error))?;

        let config = Self {
            version: value.version,
            project_name,
            store_path,
        };

        config.validate()?;
        Ok(config)
    }
}

impl From<Config> for ConfigFile {
    fn from(value: Config) -> Self {
        value.to_file()
    }
}

/// Options collected during onboarding before a full config exists.
///
/// This is useful for interactive CLI flows where the user progressively enters
/// information and only receives a final validated config at the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDraft {
    /// Candidate project name.
    pub project_name: ProjectName,
    /// Candidate store path.
    pub store_path: StorePath,
}

impl ConfigDraft {
    /// Creates a new draft from validated components.
    #[must_use]
    pub const fn new(project_name: ProjectName, store_path: StorePath) -> Self {
        Self {
            project_name,
            store_path,
        }
    }

    /// Finalizes the draft into a complete config.
    pub fn finalize(self) -> Result<Config> {
        Config::new(self.project_name, self.store_path)
    }
}

/// Resolves the current working directory into a conventional project-local config path.
pub fn resolve_local_config_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().map_err(AgentMemoryError::from)?;
    Ok(Config::project_config_path(current_dir))
}

/// Resolves the current working directory into a conventional project-local store path.
pub fn resolve_local_store_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().map_err(AgentMemoryError::from)?;
    Ok(Config::project_store_path(current_dir))
}

/// Writes a file atomically by writing to a temporary sibling and then renaming.
///
/// This avoids many classes of partial-write corruption.
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        ValidationError::invalid_path("config_path", "target path must have a parent directory")
    })?;

    let file_name = target.file_name().ok_or_else(|| {
        ValidationError::invalid_path("config_path", "target path must have a file name")
    })?;

    let temp_name = format!(".{}.tmp", file_name.to_string_lossy());
    let temp_path = parent.join(temp_name);

    fs::write(&temp_path, bytes).map_err(|source| AgentMemoryError::Io {
        source: io::Error::new(
            source.kind(),
            format!(
                "failed to write temporary config file {}: {source}",
                temp_path.display()
            ),
        ),
    })?;

    fs::rename(&temp_path, target).map_err(|source| AgentMemoryError::Io {
        source: io::Error::new(
            source.kind(),
            format!(
                "failed to atomically rename {} to {}: {source}",
                temp_path.display(),
                target.display()
            ),
        ),
    })?;

    Ok(())
}

fn map_project_name_error(error: AgentMemoryError) -> AgentMemoryError {
    match error {
        AgentMemoryError::Validation(ValidationError::Empty { .. }) => {
            ConfigError::MissingField {
                field: "project_name",
            }
            .into()
        }
        AgentMemoryError::Validation(other) => ConfigError::InvalidProjectName {
            reason: validation_reason(&other),
        }
        .into(),
        other => other,
    }
}

fn map_store_path_error(error: AgentMemoryError) -> AgentMemoryError {
    match error {
        AgentMemoryError::Validation(ValidationError::Empty { .. }) => {
            ConfigError::MissingField { field: "store_path" }.into()
        }
        AgentMemoryError::Validation(other) => ConfigError::InvalidStorePath {
            reason: validation_reason(&other),
        }
        .into(),
        other => other,
    }
}

fn validation_reason(error: &ValidationError) -> &'static str {
    match error {
        ValidationError::Empty { .. } => "value must not be empty",
        ValidationError::TooLong { .. } => "value exceeds maximum length",
        ValidationError::TooShort { .. } => "value is below minimum length",
        ValidationError::InvalidCharacter { .. } => "value contains invalid characters",
        ValidationError::InvalidEncoding { .. } => "value contains invalid encoding",
        ValidationError::InvalidPath { reason, .. } => reason,
        ValidationError::InvalidSegment { reason, .. } => reason,
        ValidationError::InvalidFormat { reason, .. } => reason,
    }
}