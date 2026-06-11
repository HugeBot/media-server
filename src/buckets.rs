use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::error::AppError;

const MIN_DIMENSION: u32 = 16;
const MAX_DIMENSION: u32 = 4096;

#[derive(Debug, Deserialize)]
struct BucketsFile {
    bucket: Vec<BucketEntry>,
}

#[derive(Debug, Deserialize)]
struct BucketEntry {
    name: String,
    max_dimension: u32,
    max_age_days: Option<u64>,
}

#[derive(Debug)]
pub struct BucketConfig {
    pub name: String,
    pub max_dimension: u32,
    pub max_age: Option<Duration>,
}

pub struct Buckets(HashMap<String, BucketConfig>);

impl Buckets {
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

    pub fn get(&self, name: &str) -> Result<&BucketConfig, AppError> {
        self.0.get(name).ok_or(AppError::InvalidBucket)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BucketConfig> {
        self.0.values()
    }
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
