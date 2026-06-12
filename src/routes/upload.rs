//! `POST /upload` — protected route that accepts a multipart image upload,
//! resizes and re-encodes it as lossless WebP, and stores it in the
//! requested bucket.

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

/// Content types accepted for the `image` field, matching the decoders
/// enabled in `image`'s Cargo features (jpeg, png, gif, webp).
const ALLOWED_IMAGE_CONTENT_TYPES: &[&str] =
    &["image/jpeg", "image/png", "image/gif", "image/webp"];

/// JSON body returned on a successful upload.
#[derive(Serialize)]
#[serde(untagged)]
pub enum UploadResponse {
    /// A regular image, stored under a fresh `UUIDv7`.
    Image {
        /// Name of the bucket the image was stored in.
        bucket: String,
        /// `UUIDv7` identifier assigned to the stored image.
        image_id: String,
        /// Fully-qualified public URL from which the image can be fetched
        /// (`GET /{bucket}/{image_id}`).
        url: String,
    },
    /// The bucket's fallback image (see [`crate::buckets::DEFAULT_IMAGE_FILENAME`]),
    /// stored when the `is_default` field is set.
    Default {
        /// Name of the bucket whose fallback image was set.
        bucket: String,
        /// Always `true`.
        default_image: bool,
    },
}

/// Handles `POST /upload`.
///
/// Expects a `multipart/form-data` body with:
/// - `bucket` (required): name of an existing bucket from `buckets.toml`.
/// - `image` (required): the image file, with a `Content-Type` of
///   `image/jpeg`, `image/png`, `image/gif` or `image/webp`.
/// - `max_dimension_override` (optional): resize to this many pixels on the
///   longest side instead of the bucket's configured `max_dimension`. Must
///   be within [`MIN_DIMENSION`]..=[`MAX_DIMENSION`], and is capped at the
///   bucket's `max_dimension` (it can only shrink the output, never enlarge
///   it beyond what the bucket allows).
/// - `is_default` (optional): if set to `true`, the image is stored as the
///   bucket's fallback image (`{STORAGE_DIR}/{bucket}/_default.webp`,
///   see [`crate::buckets::DEFAULT_IMAGE_FILENAME`]) instead of a new
///   `{uuid}.webp`, overwriting any previous fallback.
///
/// On success, the image is decoded, resized and re-encoded as lossless
/// WebP. Image processing runs inside `spawn_blocking` since it is CPU-bound
/// and would otherwise block the async runtime. Unless `is_default` is set,
/// it is written to `{STORAGE_DIR}/{bucket}/{uuid}.webp` where `uuid` is a
/// fresh `UUIDv7`.
pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    let mut bucket: Option<String> = None;
    let mut image_bytes: Option<Bytes> = None;
    let mut max_dimension_override: Option<u32> = None;
    let mut is_default = false;

    // Multipart fields can arrive in any order, so collect them all before
    // validating that the required ones are present.
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
                let content_type = field.content_type().unwrap_or_default();
                if !ALLOWED_IMAGE_CONTENT_TYPES.contains(&content_type) {
                    return Err(AppError::BadRequest(format!(
                        "unsupported image content type '{content_type}', expected one of {ALLOWED_IMAGE_CONTENT_TYPES:?}"
                    )));
                }

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
                    AppError::BadRequest("max_dimension_override must be a positive integer".into())
                })?;
                if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&value) {
                    return Err(AppError::BadRequest(format!(
                        "max_dimension_override must be between {MIN_DIMENSION} and {MAX_DIMENSION}"
                    )));
                }
                max_dimension_override = Some(value);
            }
            Some("is_default") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("invalid is_default field: {e}")))?;
                is_default = text.trim().eq_ignore_ascii_case("true") || text.trim() == "1";
            }
            // Ignore any other fields the client may send.
            _ => {}
        }
    }

    let bucket = bucket.ok_or_else(|| AppError::BadRequest("missing bucket field".into()))?;
    let image_bytes =
        image_bytes.ok_or_else(|| AppError::BadRequest("missing image field".into()))?;

    let bucket_cfg = config.buckets.get(&bucket)?;

    // An override can only make the output smaller than (or equal to) the
    // bucket's configured limit, never larger.
    let max_dimension = max_dimension_override.map_or(bucket_cfg.max_dimension, |override_value| {
        override_value.min(bucket_cfg.max_dimension)
    });

    let bytes_len = image_bytes.len();
    let webp = tokio::task::spawn_blocking(move || process_image(&image_bytes, max_dimension))
        .await
        .expect("image processing task panicked")?;

    if is_default {
        let path = bucket_cfg.default_image_path(&config.storage_dir);
        tokio::fs::write(&path, webp).await?;

        tracing::info!(
            bucket = bucket_cfg.name,
            bytes = bytes_len,
            "stored bucket fallback image"
        );

        return Ok(Json(UploadResponse::Default {
            bucket: bucket_cfg.name.clone(),
            default_image: true,
        }));
    }

    let image_id = Uuid::now_v7();

    let path = bucket_cfg.image_path(&config.storage_dir, image_id);
    tokio::fs::write(&path, webp).await?;

    tracing::info!(
        bucket = bucket_cfg.name,
        image_id = %image_id,
        bytes = bytes_len,
        "stored uploaded image"
    );

    Ok(Json(UploadResponse::Image {
        bucket: bucket_cfg.name.clone(),
        image_id: image_id.to_string(),
        url: format!(
            "{}/{}/{}",
            config.public_base_url, bucket_cfg.name, image_id
        ),
    }))
}
