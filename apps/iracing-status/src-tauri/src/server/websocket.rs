use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tauri::AppHandle;

use super::{
    settings::TransportSettings,
    transport::{start_axum_transport, ServerHandle, ACCEPT_POLL_INTERVAL},
};
use crate::state::{ConnectionStateObserver, IRacingConnectionState};

const ASYNCAPI_SCHEMA: &str = include_str!("../../../docs/specs/asyncapi.yaml");

#[derive(Clone, Debug)]
pub(super) struct WebsocketRuntime {
    app: Option<AppHandle>,
    observer: Arc<ConnectionStateObserver>,
}

impl WebsocketRuntime {
    pub(super) fn new(app: AppHandle, observer: Arc<ConnectionStateObserver>) -> Self {
        Self {
            app: Some(app),
            observer,
        }
    }
}

pub(super) fn start_websocket_server(
    settings: TransportSettings,
    runtime: Option<WebsocketRuntime>,
) -> Result<ServerHandle, String> {
    tracing::debug!(
        settings = ?settings,
        runtime_available = runtime.is_some(),
        "starting WebSocket server"
    );
    start_axum_transport(settings, "WebSocket", "ws", move |shutdown| {
        router(shutdown, runtime)
    })
}

fn router(shutdown: Arc<AtomicBool>, runtime: Option<WebsocketRuntime>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/schema", get(schema))
        .route("/status", get(status_upgrade))
        .route("/session", get(not_implemented_websocket_upgrade))
        .route("/telemetry", get(not_implemented_websocket_upgrade))
        .route("/capabilities", get(not_implemented_websocket_upgrade))
        .route("/available-schema", get(not_implemented_websocket_upgrade))
        .fallback(not_found)
        .with_state(WebsocketState { shutdown, runtime })
}

#[derive(Clone)]
struct WebsocketState {
    shutdown: Arc<AtomicBool>,
    runtime: Option<WebsocketRuntime>,
}

async fn root(uri: Uri) -> &'static str {
    tracing::debug!(route = %uri, "WebSocket HTTP request");
    "iRacing status WebSocket server\n"
}

async fn health(uri: Uri) -> Json<serde_json::Value> {
    tracing::debug!(route = %uri, "WebSocket HTTP request");
    Json(json!({ "status": "ok", "service": "iracing-status" }))
}

async fn schema(uri: Uri) -> Response {
    tracing::debug!(route = %uri, "WebSocket HTTP request");
    (
        StatusCode::OK,
        [("content-type", "application/yaml")],
        ASYNCAPI_SCHEMA,
    )
        .into_response()
}

async fn status_upgrade(
    uri: Uri,
    State(state): State<WebsocketState>,
    websocket: WebSocketUpgrade,
) -> Response {
    tracing::debug!(
        route = %uri,
        runtime_available = state.runtime.is_some(),
        "WebSocket upgrade request"
    );
    websocket
        .on_upgrade(move |socket| handle_status_connection(socket, state))
        .into_response()
}

async fn not_implemented_websocket_upgrade(uri: Uri, websocket: WebSocketUpgrade) -> Response {
    tracing::debug!(route = %uri, "WebSocket upgrade request for unimplemented route");
    let route = uri.to_string();
    websocket
        .on_upgrade(move |socket| handle_not_implemented_connection(socket, route))
        .into_response()
}

async fn handle_status_connection(mut socket: WebSocket, state: WebsocketState) {
    tracing::debug!("WebSocket status connection opened");
    let Some(runtime) = state.runtime else {
        tracing::debug!("WebSocket status connection rejected; runtime unavailable");
        let _ = socket
            .send(Message::Text(
                json!({
                    "error": "Connection-state streaming is unavailable in this runtime."
                })
                .to_string()
                .into(),
            ))
            .await;
        return;
    };

    let current_state = match runtime.app.clone() {
        Some(app) => {
            tracing::debug!("ensuring connection-state observer is started for WebSocket stream");
            runtime.observer.ensure_started(app)
        }
        None => {
            tracing::debug!("using existing connection-state snapshot for WebSocket stream");
            runtime.observer.current_state()
        }
    };

    if send_connection_state(&mut socket, current_state)
        .await
        .is_err()
    {
        tracing::debug!("WebSocket status connection closed before initial state was sent");
        return;
    }

    let mut updates = runtime.observer.subscribe();
    while !state.shutdown.load(Ordering::Acquire) {
        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    tracing::debug!("WebSocket status updates channel closed");
                    break;
                }

                let next_state = *updates.borrow();
                if send_connection_state(&mut socket, next_state).await.is_err() {
                    tracing::debug!(
                        state = ?next_state,
                        "WebSocket status connection closed while sending state"
                    );
                    break;
                }
            }
            incoming = tokio::time::timeout(ACCEPT_POLL_INTERVAL, socket.recv()) => {
                match incoming {
                    Ok(Some(Ok(Message::Close(frame)))) => {
                        tracing::debug!(close_frame = ?frame, "WebSocket status connection close received");
                        break;
                    }
                    Ok(None) => {
                        tracing::debug!("WebSocket status connection ended");
                        break;
                    }
                    Ok(Some(Ok(message))) => {
                        tracing::debug!(message = ?message, "WebSocket status inbound message ignored");
                    }
                    Ok(Some(Err(error))) => {
                        tracing::debug!(error = %error, "WebSocket status receive error");
                        break;
                    }
                    Err(_) => {}
                }
            }
        }
    }

    tracing::debug!("WebSocket status connection closed");
}

async fn handle_not_implemented_connection(mut socket: WebSocket, route: String) {
    tracing::debug!(route = %route, "WebSocket unimplemented route connection opened");
    let _ = socket
        .send(Message::Text(
            json!({
                "error": "This WebSocket route is documented but not implemented yet."
            })
            .to_string()
            .into(),
        ))
        .await;
    tracing::debug!(route = %route, "WebSocket unimplemented route response sent");
}

async fn send_connection_state(
    socket: &mut WebSocket,
    state: IRacingConnectionState,
) -> Result<(), ()> {
    let payload = serde_json::to_string(&state).map_err(|_| ())?;
    tracing::debug!(state = ?state, "sending WebSocket connection state");
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

async fn not_found(uri: Uri) -> Response {
    tracing::debug!(route = %uri, "WebSocket HTTP request not found");
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}
