//! HTTP route handlers.
//!
//! - [`upload`]: `POST /upload` — protected, stores a resized WebP image.
//! - [`serve`]: `GET /{bucket}/{image_id}` — public, streams a stored image.
//! - [`delete`]: `DELETE /{bucket}/{image_id}` — protected, removes a stored
//!   image.
//!
//! Routing and the protected/public split (via [`crate::auth::require_token`])
//! are wired up in `src/main.rs`.

pub mod delete;
pub mod serve;
pub mod upload;
