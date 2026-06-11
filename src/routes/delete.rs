use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::bucket::Bucket;
use crate::config::AppConfig;
use crate::error::AppError;

pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    Path((bucket, image_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let bucket: Bucket = bucket.parse()?;
    let image_id: Uuid = image_id.parse().map_err(|_| AppError::InvalidImageId)?;

    let path = config
        .storage_dir
        .join(bucket.as_str())
        .join(format!("{image_id}.webp"));

    tokio::fs::remove_file(&path).await?;

    Ok(StatusCode::NO_CONTENT)
}
