use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::ServeFile;
use uuid::Uuid;

use crate::bucket::Bucket;
use crate::config::AppConfig;
use crate::error::AppError;

/// Serves the stored WebP file, delegating to `ServeFile` for conditional
/// requests (`If-None-Match`/`If-Modified-Since` -> 304), `Range` requests,
/// and streaming the body instead of buffering it in memory.
pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    Path((bucket, image_id)): Path<(String, String)>,
    request: Request,
) -> Result<Response, AppError> {
    let bucket: Bucket = bucket.parse()?;
    let image_id: Uuid = image_id.parse().map_err(|_| AppError::InvalidImageId)?;

    let path = config
        .storage_dir
        .join(bucket.as_str())
        .join(format!("{image_id}.webp"));

    let response = ServeFile::new(&path).oneshot(request).await.unwrap();

    if response.status() == StatusCode::NOT_FOUND {
        return Err(AppError::NotFound);
    }

    let mut response = response.map(Body::new).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok(response)
}
