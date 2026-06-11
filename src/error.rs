use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    InvalidBucket,
    InvalidImageId,
    DecodeError(image::ImageError),
    EncodeError(image::ImageError),
    Io(std::io::Error),
    NotFound,
    Unauthorized,
    BadRequest(String),
}

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

        (status, Json(json!({ "error": message }))).into_response()
    }
}
