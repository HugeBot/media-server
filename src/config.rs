//! Application configuration, loaded once at startup from environment
//! variables (and, for buckets, the file pointed to by `BUCKETS_CONFIG_PATH`).

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use crate::buckets::Buckets;

/// Shared, read-only application configuration.
///
/// Built once via [`AppConfig::from_env`] and wrapped in an `Arc` so it can
/// be cheaply cloned into the Axum router state, the auth middleware, and
/// the background cleanup task.
pub struct AppConfig {
    /// Address the HTTP server listens on (`BIND_ADDR`, default
    /// `0.0.0.0:3000`).
    pub bind_addr: String,
    /// Root directory under which each bucket gets its own subdirectory
    /// (`STORAGE_DIR`, default `./storage`).
    pub storage_dir: PathBuf,
    /// Base URL used to build the `url` field in upload responses
    /// (`PUBLIC_BASE_URL`), with any trailing slash stripped.
    pub public_base_url: String,
    /// Maximum accepted request body size in bytes, enforced via
    /// `DefaultBodyLimit` (`MAX_UPLOAD_BYTES`, default 25 MiB).
    pub max_upload_bytes: usize,
    /// Bearer token required to call `/upload` and `DELETE` (`API_TOKEN`,
    /// required, no default).
    pub api_token: String,
    /// Per-bucket configuration loaded from `BUCKETS_CONFIG_PATH`. See
    /// [`crate::buckets`].
    pub buckets: Buckets,
    /// How often the background cleanup task sweeps each bucket
    /// (`CLEANUP_INTERVAL_SECS`, default 3600).
    pub cleanup_interval: Duration,
}

impl AppConfig {
    /// Reads configuration from environment variables and the bucket
    /// configuration file.
    ///
    /// # Panics
    ///
    /// Panics if `API_TOKEN` is not set, or if the bucket configuration
    /// fails to load/validate (see [`Buckets::load`]). Both are treated as
    /// fatal startup errors by design: a misconfigured server should refuse
    /// to start rather than run with unsafe defaults.
    pub fn from_env() -> Self {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        let storage_dir =
            PathBuf::from(env::var("STORAGE_DIR").unwrap_or_else(|_| "./storage".to_string()));

        let public_base_url = env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "https://static-media.huge.bot".to_string())
            .trim_end_matches('/')
            .to_string();

        let max_upload_bytes = env::var("MAX_UPLOAD_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25 * 1024 * 1024);

        let api_token = env::var("API_TOKEN").expect("API_TOKEN env var must be set");

        let buckets_config_path =
            PathBuf::from(env::var("BUCKETS_CONFIG_PATH").unwrap_or_else(|_| "./buckets.toml".to_string()));
        let buckets = Buckets::load(&buckets_config_path);

        let cleanup_interval_secs: u64 = env::var("CLEANUP_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        Self {
            bind_addr,
            storage_dir,
            public_base_url,
            max_upload_bytes,
            api_token,
            buckets,
            cleanup_interval: Duration::from_secs(cleanup_interval_secs),
        }
    }
}
