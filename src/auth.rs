//! Bearer-token authentication middleware for protected routes
//! (`POST /upload` and `DELETE /{bucket}/{image_id}`).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AppConfig;
use crate::error::AppError;

/// Axum middleware that requires a valid `Authorization: Bearer <API_TOKEN>`
/// header, rejecting the request with [`AppError::Unauthorized`] otherwise.
///
/// Applied via `route_layer` so it only runs for the routes it's attached
/// to (see `src/main.rs`), not for the public serving/health routes.
pub async fn require_token(
    State(config): State<Arc<AppConfig>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(token) if constant_time_eq(token.as_bytes(), config.api_token.as_bytes()) => {
            Ok(next.run(req).await)
        }
        _ => Err(AppError::Unauthorized),
    }
}

/// Compares two byte slices for equality in constant time (independent of
/// where the first differing byte is), to avoid leaking information about
/// the configured API token via response-time side channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .zip(b.iter())
        .fold(0u8, |diff, (x, y)| diff | (x ^ y))
        == 0
}
