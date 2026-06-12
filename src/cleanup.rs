//! Background task that periodically deletes expired images from each
//! bucket's storage directory, based on the bucket's configured
//! [`crate::buckets::BucketConfig::max_age`].

use std::sync::Arc;
use std::time::SystemTime;

use crate::config::AppConfig;

/// Spawns a background task that periodically removes files older than each
/// bucket's configured lifetime from its storage directory. Buckets without
/// a configured lifetime (`max_age = None`, i.e. permanent buckets) are
/// skipped entirely.
///
/// Runs on its own Tokio task at `config.cleanup_interval`, for the lifetime
/// of the process (the returned `JoinHandle` is not awaited; the task is
/// implicitly cancelled on shutdown).
pub fn spawn(config: Arc<AppConfig>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(config.cleanup_interval);
        loop {
            interval.tick().await;
            run_once(&config).await;
        }
    })
}

/// Performs a single cleanup pass over every configured bucket.
///
/// For each non-permanent bucket, lists `*.webp` files in its storage
/// directory and removes those whose modification time is older than the
/// bucket's `max_age`. Missing directories, unreadable entries, and files
/// that fail to delete are silently skipped (best-effort cleanup; they will
/// simply be retried on the next pass).
async fn run_once(config: &AppConfig) {
    for bucket in config.buckets.iter() {
        let Some(max_age) = bucket.max_age else {
            continue;
        };

        let dir = bucket.storage_dir(&config.storage_dir);

        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        let mut removed = 0u32;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("webp") {
                continue;
            }

            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };

            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default();

            if age > max_age && tokio::fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }

        if removed > 0 {
            tracing::info!(
                bucket = bucket.name,
                removed,
                "cleanup: removed expired file(s)"
            );
        }
    }
}
