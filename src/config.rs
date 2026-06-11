use std::env;
use std::path::PathBuf;
use std::time::Duration;

pub struct AppConfig {
    pub bind_addr: String,
    pub storage_dir: PathBuf,
    pub public_base_url: String,
    pub max_upload_bytes: usize,
    pub api_token: String,
    pub max_age: Duration,
    pub cleanup_interval: Duration,
}

impl AppConfig {
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

        let max_age_days: u64 = env::var("MAX_AGE_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15);

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
            max_age: Duration::from_secs(max_age_days * 24 * 60 * 60),
            cleanup_interval: Duration::from_secs(cleanup_interval_secs),
        }
    }
}
