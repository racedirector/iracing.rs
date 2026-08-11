//! WebSocket facade for the iRacing SDK.
//!
//! Runtime setup belongs in the `iracing-sdk-ws` binary. This library only
//! defines the HTTP router and WebSocket boundary.

use axum::{
    Router,
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
};

/// Path used by the WebSocket endpoint.
pub const WEBSOCKET_PATH: &str = "/ws";

/// Builds the WebSocket facade router.
pub fn router() -> Router {
    Router::new().route(WEBSOCKET_PATH, get(upgrade_websocket))
}

async fn upgrade_websocket(upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while socket.recv().await.is_some() {}
}
