use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use serde::Serialize;
use uuid::Uuid;

use crate::buckets::{MAX_DIMENSION, MIN_DIMENSION};
use crate::config::AppConfig;
use crate::error::AppError;
use crate::image_processing::process_image;

#[derive(Serialize)]
pub struct UploadResponse {
    bucket: String,
    image_id: String,
    url: String,
}

pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    let mut bucket: Option<String> = None;
    let mut image_bytes: Option<Bytes> = None;
    let mut max_dimension_override: Option<u32> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("invalid multipart payload: {e}")))?
    {
        match field.name() {
            Some("bucket") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("invalid bucket field: {e}")))?;
                bucket = Some(text);
            }
            Some("image") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("invalid image field: {e}")))?;
                image_bytes = Some(bytes);
            }
            Some("max_dimension_override") => {
                let text = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("invalid max_dimension_override field: {e}"))
                })?;
                let value: u32 = text.parse().map_err(|_| {
                    AppError::BadRequest(
                        "max_dimension_override must be a positive integer".into(),
                    )
                })?;
                if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&value) {
                    return Err(AppError::BadRequest(format!(
                        "max_dimension_override must be between {MIN_DIMENSION} and {MAX_DIMENSION}"
                    )));
                }
                max_dimension_override = Some(value);
            }
            _ => {}
        }
    }

    let bucket = bucket.ok_or_else(|| AppError::BadRequest("missing bucket field".into()))?;
    let image_bytes =
        image_bytes.ok_or_else(|| AppError::BadRequest("missing image field".into()))?;

    let bucket_cfg = config.buckets.get(&bucket)?;
    let max_dimension = match max_dimension_override {
        Some(override_value) => override_value.min(bucket_cfg.max_dimension),
        None => bucket_cfg.max_dimension,
    };

    let bytes_len = image_bytes.len();
    let webp = tokio::task::spawn_blocking(move || process_image(&image_bytes, max_dimension))
        .await
        .expect("image processing task panicked")?;

    let image_id = Uuid::now_v7();

    let path = config
        .storage_dir
        .join(&bucket_cfg.name)
        .join(format!("{image_id}.webp"));
    tokio::fs::write(&path, webp).await?;

    tracing::info!(
        bucket = bucket_cfg.name,
        image_id = %image_id,
        bytes = bytes_len,
        "stored uploaded image"
    );

    Ok(Json(UploadResponse {
        bucket: bucket_cfg.name.clone(),
        image_id: image_id.to_string(),
        url: format!(
            "{}/{}/{}",
            config.public_base_url,
            bucket_cfg.name,
            image_id
        ),
    }))
}
