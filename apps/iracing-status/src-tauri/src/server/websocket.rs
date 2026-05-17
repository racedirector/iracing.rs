use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    Router,
};

use super::{
    settings::TransportSettings,
    transport::{start_listener_transport, ServerHandle, ACCEPT_POLL_INTERVAL},
};

pub(super) fn start_websocket_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    start_listener_transport(settings, "WebSocket", "ws", run_websocket_server)
}

fn run_websocket_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
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

        let _ = axum::serve(listener, router(Arc::clone(&shutdown)))
            .with_graceful_shutdown(async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
            })
            .await;
    });
}

fn router(shutdown: Arc<AtomicBool>) -> Router {
    Router::new()
        .fallback(websocket_upgrade)
        .with_state(shutdown)
}

async fn websocket_upgrade(
    State(shutdown): State<Arc<AtomicBool>>,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .on_upgrade(move |socket| handle_websocket_connection(socket, shutdown))
        .into_response()
}

async fn handle_websocket_connection(mut socket: WebSocket, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        match tokio::time::timeout(ACCEPT_POLL_INTERVAL, socket.recv()).await {
            Ok(Some(Ok(_message))) => {}
            Ok(Some(Err(_))) | Ok(None) => break,
            Err(_) => {}
        }
    }
}
