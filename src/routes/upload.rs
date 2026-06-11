use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use serde::Serialize;
use uuid::Uuid;

use crate::bucket::Bucket;
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
    let mut bucket: Option<Bucket> = None;
    let mut image_bytes: Option<Bytes> = None;

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
                bucket = Some(text.parse()?);
            }
            Some("image") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("invalid image field: {e}")))?;
                image_bytes = Some(bytes);
            }
            _ => {}
        }
    }

    let bucket = bucket.ok_or_else(|| AppError::BadRequest("missing bucket field".into()))?;
    let image_bytes =
        image_bytes.ok_or_else(|| AppError::BadRequest("missing image field".into()))?;

    let bytes_len = image_bytes.len();
    let webp = tokio::task::spawn_blocking(move || process_image(&image_bytes))
        .await
        .expect("image processing task panicked")?;

    let image_id = Uuid::now_v7();

    let path = config
        .storage_dir
        .join(bucket.as_str())
        .join(format!("{image_id}.webp"));
    tokio::fs::write(&path, webp).await?;

    tracing::info!(
        bucket = bucket.as_str(),
        image_id = %image_id,
        bytes = bytes_len,
        "stored uploaded image"
    );

    Ok(Json(UploadResponse {
        bucket: bucket.as_str().to_string(),
        image_id: image_id.to_string(),
        url: format!(
            "{}/{}/{}",
            config.public_base_url,
            bucket.as_str(),
            image_id
        ),
    }))
}
