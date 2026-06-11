use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AppConfig;
use crate::error::AppError;

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
        Some(token) if token == config.api_token => Ok(next.run(req).await),
        _ => Err(AppError::Unauthorized),
    }
}
