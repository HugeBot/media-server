use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;

pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    Path((bucket, image_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let bucket_cfg = config.buckets.get(&bucket)?;
    let image_id: Uuid = image_id.parse().map_err(|_| AppError::InvalidImageId)?;

    let path = config
        .storage_dir
        .join(&bucket_cfg.name)
        .join(format!("{image_id}.webp"));

    tokio::fs::remove_file(&path).await?;

    Ok(StatusCode::NO_CONTENT)
}
