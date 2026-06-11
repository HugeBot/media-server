mod auth;
mod bucket;
mod cleanup;
mod config;
mod error;
mod image_processing;
mod routes;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, MatchedPath, Request};
use axum::middleware;
use axum::routing::{delete, get, post};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::LatencyUnit;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{Level, Span};
use tracing_subscriber::EnvFilter;

use config::AppConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Arc::new(AppConfig::from_env());

    for bucket in bucket::Bucket::ALL {
        let dir = config.storage_dir.join(bucket.as_str());
        tokio::fs::create_dir_all(&dir)
            .await
            .unwrap_or_else(|e| panic!("failed to create storage dir {}: {e}", dir.display()));
    }

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
        .with_state(config.clone())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request<_>| {
                    let route = req
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str)
                        .unwrap_or(req.uri().path());

                    tracing::info_span!(
                        "request",
                        method = %req.method(),
                        route,
                    )
                })
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Millis),
                )
                .on_failure(|error, latency: Duration, _span: &Span| {
                    tracing::error!(?error, ?latency, "request failed at the transport layer")
                }),
        );

    let listener = TcpListener::bind(&config.bind_addr).await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

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

    tracing::info!("shutdown signal received");
}
