//! `DELETE /{bucket}/{image_id}` — protected route that removes a stored
//! image.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;

/// Handles `DELETE /{bucket}/{image_id}`.
///
/// Removes `{STORAGE_DIR}/{bucket}/{image_id}.webp`. Returns `204 No
/// Content` on success, and also `204 No Content` if the file does not
/// exist — deleting an already-deleted (or never-existing) image is treated
/// as success so the endpoint is idempotent. Returns `400 Bad Request` if
/// `bucket` or `image_id` are invalid.
pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    Path((bucket, image_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let bucket_cfg = config.buckets.get(&bucket)?;
    let image_id: Uuid = image_id.parse().map_err(|_| AppError::InvalidImageId)?;

    let path = bucket_cfg.image_path(&config.storage_dir, image_id);

    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(e.into()),
    }
}
