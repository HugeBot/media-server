mod auth;
mod bucket;
mod cleanup;
mod config;
mod error;
mod image_processing;
mod routes;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{delete, get, post};
use tokio::net::TcpListener;
use tokio::signal;

use config::AppConfig;

#[tokio::main]
async fn main() {
    let config = Arc::new(AppConfig::from_env());

    cleanup::spawn(config.clone());

    let protected = Router::new()
        .route("/upload", post(routes::upload::handler))
        .route("/{bucket}/{image_id}", delete(routes::delete::handler))
        .route_layer(middleware::from_fn_with_state(
            config.clone(),
            auth::require_token,
        ));

    let public = Router::new().route("/{bucket}/{image_id}", get(routes::serve::handler));

    let app = protected
        .merge(public)
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
        .with_state(config.clone());

    let listener = TcpListener::bind(&config.bind_addr).await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("shutdown signal received");
}
