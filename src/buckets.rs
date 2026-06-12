//! Bucket configuration, loaded from a TOML file (see [`Buckets::load`]).
//!
//! Each bucket controls two things for the images stored in it:
//! - [`BucketConfig::max_dimension`]: the maximum size (in pixels) of the
//!   longest side after resizing on upload.
//! - [`BucketConfig::max_age`]: how long files live before the background
//!   cleanup task ([`crate::cleanup`]) removes them. `None` means the bucket
//!   is permanent and cleanup never touches it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;

/// Smallest accepted value for `max_dimension` and `max_dimension_override`.
pub const MIN_DIMENSION: u32 = 16;
/// Largest accepted value for `max_dimension` and `max_dimension_override`.
pub const MAX_DIMENSION: u32 = 4096;

/// Raw shape of the `buckets.toml` file.
#[derive(Debug, Deserialize)]
struct BucketsFile {
    bucket: Vec<BucketEntry>,
}

/// Raw, unvalidated entry for a single `[[bucket]]` table in `buckets.toml`.
#[derive(Debug, Deserialize)]
struct BucketEntry {
    name: String,
    max_dimension: u32,
    /// Lifetime in days. Omitted/absent means the bucket is permanent.
    max_age_days: Option<u64>,
}

/// Validated configuration for a single bucket.
#[derive(Debug)]
pub struct BucketConfig {
    /// Bucket name. Used as the storage subdirectory name and as the
    /// `{bucket}` path segment in routes. Guaranteed to be a safe path
    /// component (see [`is_valid_name`]).
    pub name: String,
    /// Maximum length, in pixels, of the longest side of stored images.
    pub max_dimension: u32,
    /// How long files in this bucket live before cleanup removes them.
    /// `None` means the bucket is permanent (cleanup skips it).
    pub max_age: Option<Duration>,
}

impl BucketConfig {
    /// Returns this bucket's storage directory under `storage_root`
    /// (i.e. `{storage_root}/{bucket_name}`).
    pub fn storage_dir(&self, storage_root: &Path) -> PathBuf {
        storage_root.join(&self.name)
    }

    /// Returns the on-disk path for the stored WebP file with the given
    /// `image_id` (i.e. `{storage_root}/{bucket_name}/{image_id}.webp`).
    pub fn image_path(&self, storage_root: &Path, image_id: Uuid) -> PathBuf {
        self.storage_dir(storage_root)
            .join(format!("{image_id}.webp"))
    }
}

/// All configured buckets, keyed by name.
///
/// Built once at startup via [`Buckets::load`] and shared (read-only)
/// through [`crate::config::AppConfig`].
pub struct Buckets(HashMap<String, BucketConfig>);

impl Buckets {
    /// Reads and validates the bucket configuration file at `path`.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if the file cannot be read, is not
    /// valid TOML, defines zero buckets, or contains an invalid bucket:
    /// - `name` is empty, not lowercase-alphanumeric-with-hyphens, or
    ///   starts/ends with a hyphen (it is used directly as a filesystem path
    ///   component).
    /// - `name` is duplicated across entries.
    /// - `max_dimension` is outside [`MIN_DIMENSION`]..=[`MAX_DIMENSION`].
    /// - `max_age_days` is present but `0`.
    ///
    /// This is intentional: an invalid configuration should prevent the
    /// server from starting rather than fail later at request time.
    pub fn load(path: &Path) -> Self {
        let contents = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read buckets config {}: {e}", path.display()));

        let file: BucketsFile = toml::from_str(&contents)
            .unwrap_or_else(|e| panic!("failed to parse buckets config {}: {e}", path.display()));

        if file.bucket.is_empty() {
            panic!("buckets config {} must define at least one bucket", path.display());
        }

        let mut buckets = HashMap::with_capacity(file.bucket.len());
        for entry in file.bucket {
            if !is_valid_name(&entry.name) {
                panic!(
                    "invalid bucket name '{}': must be lowercase alphanumeric with hyphens, \
                     not empty, and not start/end with a hyphen",
                    entry.name
                );
            }

            if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&entry.max_dimension) {
                panic!(
                    "invalid max_dimension {} for bucket '{}': must be between {} and {}",
                    entry.max_dimension, entry.name, MIN_DIMENSION, MAX_DIMENSION
                );
            }

            let max_age = match entry.max_age_days {
                Some(0) => panic!("invalid max_age_days for bucket '{}': must be >= 1", entry.name),
                Some(days) => Some(Duration::from_secs(days * 24 * 60 * 60)),
                None => None,
            };

            let config = BucketConfig {
                name: entry.name.clone(),
                max_dimension: entry.max_dimension,
                max_age,
            };

            if buckets.insert(entry.name.clone(), config).is_some() {
                panic!("duplicate bucket name '{}'", entry.name);
            }
        }

        Self(buckets)
    }

    /// Looks up a bucket by name.
    ///
    /// Returns [`AppError::InvalidBucket`] if no bucket with that name is
    /// configured. Route handlers use this both to validate the `{bucket}`
    /// path/form segment and to obtain the canonical [`BucketConfig`].
    pub fn get(&self, name: &str) -> Result<&BucketConfig, AppError> {
        self.0.get(name).ok_or(AppError::InvalidBucket)
    }

    /// Iterates over all configured buckets, in arbitrary order.
    ///
    /// Used at startup to create storage directories and by the cleanup
    /// task to sweep each bucket independently.
    pub fn iter(&self) -> impl Iterator<Item = &BucketConfig> {
        self.0.values()
    }
}

/// Validates a bucket name for safe use as a single filesystem path
/// component: lowercase ASCII letters, digits and hyphens only, non-empty,
/// and not starting or ending with a hyphen.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
