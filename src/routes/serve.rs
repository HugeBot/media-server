//! `GET /{bucket}/{image_id}` — public route that streams a stored WebP
//! image.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::ServeFile;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;

/// Serves the stored WebP file, delegating to `ServeFile` for conditional
/// requests (`If-None-Match`/`If-Modified-Since` -> 304), `Range` requests,
/// and streaming the body instead of buffering it in memory.
///
/// Adds a one-year `immutable` `Cache-Control` header on success, since
/// stored files are content-addressed by UUID and never modified in place.
///
/// If `{image_id}.webp` doesn't exist, falls back to the bucket's optional
/// [`crate::buckets::DEFAULT_IMAGE_FILENAME`] (`_default.webp`), if present,
/// served with `200 OK` and a short `Cache-Control` (since the same
/// `image_id` may start resolving to a real file shortly after, e.g. once a
/// Twitch stream goes live). If neither exists, returns
/// [`AppError::NotFound`].
pub async fn handler(
    State(config): State<Arc<AppConfig>>,
    Path((bucket, image_id)): Path<(String, String)>,
    request: Request,
) -> Result<Response, AppError> {
    let bucket_cfg = config.buckets.get(&bucket)?;
    let image_id: Uuid = image_id.parse().map_err(|_| AppError::InvalidImageId)?;

    let path = bucket_cfg.image_path(&config.storage_dir, image_id);

    // Keep the method and conditional/range headers around in case we need
    // to retry against the fallback image below; `request` is consumed by
    // `oneshot`.
    let method = request.method().clone();
    let headers = request.headers().clone();

    // `ServeFile`'s `Service::Error` is `Infallible`: missing files and I/O
    // errors are reported via the response status, not as a `Result::Err`.
    let response = ServeFile::new(&path).oneshot(request).await.unwrap();

    if response.status() == StatusCode::NOT_FOUND {
        let default_path = bucket_cfg.default_image_path(&config.storage_dir);

        // Rebuild a request carrying the original method and
        // conditional/range headers, since `Request` isn't `Clone`.
        let mut fallback_request = Request::new(Body::empty());
        *fallback_request.method_mut() = method;
        *fallback_request.headers_mut() = headers;

        let fallback_response = ServeFile::new(&default_path)
            .oneshot(fallback_request)
            .await
            .unwrap();

        if fallback_response.status() == StatusCode::NOT_FOUND {
            return Err(AppError::NotFound);
        }

        let mut response = fallback_response.map(Body::new).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=30"),
        );

        return Ok(response);
    }

    let mut response = response.map(Body::new).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok(response)
}
