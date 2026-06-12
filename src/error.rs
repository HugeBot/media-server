//! Central error type for the application.
//!
//! [`AppError`] is the error type returned by every route handler and
//! middleware. Its [`IntoResponse`] implementation maps each variant to an
//! HTTP status code and a `{"error": "..."}` JSON body, and logs the error
//! via `tracing` (at `error` level for 5xx, `warn` for 4xx) so production
//! issues are visible in the logs without leaking internals to clients.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// All error conditions that can occur while handling a request.
#[derive(Debug)]
pub enum AppError {
    /// The `{bucket}` path/form value does not match any bucket configured
    /// in `buckets.toml`. Maps to `400 Bad Request`.
    InvalidBucket,
    /// The `{image_id}` path segment is not a valid UUID. Maps to
    /// `400 Bad Request`.
    InvalidImageId,
    /// The uploaded bytes could not be decoded as a supported image format.
    /// Maps to `400 Bad Request` since the input is at fault.
    DecodeError(image::ImageError),
    /// Re-encoding the (valid) decoded image as WebP failed. Maps to
    /// `500 Internal Server Error` since this indicates a server-side bug
    /// or resource exhaustion, not bad input.
    EncodeError(image::ImageError),
    /// Any other I/O error (reading/writing/removing files). Note that
    /// `NotFound` I/O errors are converted to [`AppError::NotFound`] instead
    /// via the `From` impl below. Maps to `500 Internal Server Error`.
    Io(std::io::Error),
    /// The requested image file does not exist. Maps to `404 Not Found`.
    NotFound,
    /// Missing or incorrect `Authorization: Bearer <token>` header on a
    /// protected route. Maps to `401 Unauthorized`.
    Unauthorized,
    /// Malformed request (bad multipart payload, missing fields, invalid
    /// `max_dimension_override`, etc.). The `String` is the message returned
    /// to the client, so it must not contain sensitive information. Maps to
    /// `400 Bad Request`.
    BadRequest(String),
}

/// Converts filesystem I/O errors into [`AppError`], turning "file does not
/// exist" into [`AppError::NotFound`] (a normal, expected outcome for
/// `GET`/`DELETE` on a missing image) and everything else into
/// [`AppError::Io`] (an unexpected server-side failure).
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Io(err)
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::InvalidBucket => (StatusCode::BAD_REQUEST, "invalid bucket".to_string()),
            AppError::InvalidImageId => {
                (StatusCode::BAD_REQUEST, "invalid image id".to_string())
            }
            AppError::DecodeError(e) => {
                (StatusCode::BAD_REQUEST, format!("could not decode image: {e}"))
            }
            AppError::EncodeError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not encode image: {e}"),
            ),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("io error: {e}")),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        // Log server errors loudly (they indicate a bug or infra issue) and
        // client errors quietly (expected, e.g. bad input or missing auth),
        // so production issues surface in `tracing::error!` without
        // drowning logs in routine 4xx noise.
        if status.is_server_error() {
            tracing::error!(error = ?self, status = %status, "request failed");
        } else {
            tracing::warn!(error = ?self, status = %status, "request rejected");
        }

        (status, Json(json!({ "error": message }))).into_response()
    }
}
