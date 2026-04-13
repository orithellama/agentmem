//! Storage format versioning and migrations.
//!
//! The project should never silently reinterpret older on-disk formats.
//! This module defines:
//!
//! - current store format version
//! - compatibility checks
//! - upgrade planning hooks
//! - migration metadata
//!
//! For v1 there is only one supported version, but establishing this boundary
//! early prevents future chaos.

use crate::error::{Result, StoreError};

/// Current supported on-disk store format version.
///
/// Increment only when the persisted structure changes in a way that requires
/// explicit compatibility handling.
pub const STORE_FORMAT_VERSION: u32 = 1;

/// Oldest readable version supported by this binary.
///
/// In v1 this equals the current version.
pub const MIN_READABLE_VERSION: u32 = 1;

/// Oldest writable version supported by this binary.
///
/// Normally equals current version unless intentionally writing legacy formats.
pub const MIN_WRITABLE_VERSION: u32 = STORE_FORMAT_VERSION;

/// Describes the compatibility state of a discovered file version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    /// Fully supported without migration.
    Current,

    /// Readable but should be upgraded.
    UpgradeRecommended,

    /// Not readable by this binary.
    UnsupportedTooOld,

    /// Newer than this binary understands.
    UnsupportedTooNew,
}

/// Human-readable migration plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    /// Source version discovered on disk.
    pub from: u32,

    /// Target version this binary wants.
    pub to: u32,

    /// Whether migration can happen automatically.
    pub automatic: bool,

    /// Summary text suitable for CLI output.
    pub message: String,
}

/// Returns the compatibility state for a discovered version.
#[must_use]
pub const fn compatibility(version: u32) -> Compatibility {
    if version == STORE_FORMAT_VERSION {
        Compatibility::Current
    } else if version >= MIN_READABLE_VERSION && version < STORE_FORMAT_VERSION {
        Compatibility::UpgradeRecommended
    } else if version < MIN_READABLE_VERSION {
        Compatibility::UnsupportedTooOld
    } else {
        Compatibility::UnsupportedTooNew
    }
}

/// Ensures a discovered version can be read.
pub fn ensure_readable(version: u32) -> Result<()> {
    match compatibility(version) {
        Compatibility::Current | Compatibility::UpgradeRecommended => Ok(()),

        Compatibility::UnsupportedTooOld | Compatibility::UnsupportedTooNew => {
            Err(StoreError::UnsupportedVersion { version }.into())
        }
    }
}

/// Ensures a requested version can be written.
pub fn ensure_writable(version: u32) -> Result<()> {
    if version < MIN_WRITABLE_VERSION || version > STORE_FORMAT_VERSION {
        return Err(StoreError::UnsupportedVersion { version }.into());
    }

    Ok(())
}

/// Creates a migration plan if one is possible.
#[must_use]
pub fn plan(from: u32) -> Option<MigrationPlan> {
    match compatibility(from) {
        Compatibility::Current => None,

        Compatibility::UpgradeRecommended => Some(MigrationPlan {
            from,
            to: STORE_FORMAT_VERSION,
            automatic: true,
            message: format!(
                "upgrade store format from version {} to {}",
                from, STORE_FORMAT_VERSION
            ),
        }),

        Compatibility::UnsupportedTooOld => Some(MigrationPlan {
            from,
            to: STORE_FORMAT_VERSION,
            automatic: false,
            message: format!("store version {} is too old for automatic migration", from),
        }),

        Compatibility::UnsupportedTooNew => Some(MigrationPlan {
            from,
            to: STORE_FORMAT_VERSION,
            automatic: false,
            message: format!("store version {} is newer than this binary supports", from),
        }),
    }
}

/// Migrates a discovered version to the current format.
///
/// In v1 there are no historical formats yet, so this is effectively a guarded
/// no-op used to establish future extension points.
pub fn migrate_version(from: u32) -> Result<u32> {
    match compatibility(from) {
        Compatibility::Current => Ok(STORE_FORMAT_VERSION),

        Compatibility::UpgradeRecommended => {
            // Future migration steps would be applied here.
            Ok(STORE_FORMAT_VERSION)
        }

        Compatibility::UnsupportedTooOld | Compatibility::UnsupportedTooNew => {
            Err(StoreError::UnsupportedVersion { version: from }.into())
        }
    }
}

/// Returns a stable human-readable description of the current format.
#[must_use]
pub fn describe_current() -> &'static str {
    "version 1: JSON snapshot file with ordered key/value records"
}
