use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use super::{
    settings::TransportSettings,
    transport::{start_listener_transport, ServerHandle, ACCEPT_POLL_INTERVAL},
};

pub(super) fn start_http_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    start_listener_transport(settings, "HTTP", "http", run_http_server)
}

fn run_http_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };

        let _ = axum::serve(listener, router())
            .with_graceful_shutdown(async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
            })
            .await;
    });
}

fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .fallback(not_found)
}

async fn root() -> &'static str {
    "iRacing status HTTP server\n"
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "iracing-status",
    })
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}
