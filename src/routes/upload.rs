use std::sync::Arc;

use axum::Json;
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
    let mut image_bytes: Option<Vec<u8>> = None;

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
                image_bytes = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    let bucket = bucket.ok_or_else(|| AppError::BadRequest("missing bucket field".into()))?;
    let image_bytes =
        image_bytes.ok_or_else(|| AppError::BadRequest("missing image field".into()))?;

    let webp = process_image(&image_bytes)?;

    let image_id = Uuid::now_v7();

    let dir = config.storage_dir.join(bucket.as_str());
    tokio::fs::create_dir_all(&dir).await?;

    let path = dir.join(format!("{image_id}.webp"));
    tokio::fs::write(&path, webp).await?;

    tracing::info!(
        bucket = bucket.as_str(),
        image_id = %image_id,
        bytes = image_bytes.len(),
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
