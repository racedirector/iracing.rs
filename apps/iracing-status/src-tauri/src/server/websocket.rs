use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
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

async fn root() -> &'static str {
    "iRacing status WebSocket server\n"
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "iracing-status" }))
}

async fn schema() -> Response {
    (
        StatusCode::OK,
        [("content-type", "application/yaml")],
        ASYNCAPI_SCHEMA,
    )
        .into_response()
}

async fn status_upgrade(
    State(state): State<WebsocketState>,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .on_upgrade(move |socket| handle_status_connection(socket, state))
        .into_response()
}

async fn not_implemented_websocket_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .on_upgrade(handle_not_implemented_connection)
        .into_response()
}

async fn handle_status_connection(mut socket: WebSocket, state: WebsocketState) {
    let Some(runtime) = state.runtime else {
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
        Some(app) => runtime.observer.ensure_started(app),
        None => runtime.observer.current_state(),
    };

    if send_connection_state(&mut socket, current_state)
        .await
        .is_err()
    {
        return;
    }

    let mut updates = runtime.observer.subscribe();
    while !state.shutdown.load(Ordering::Acquire) {
        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    break;
                }

                let next_state = *updates.borrow();
                if send_connection_state(&mut socket, next_state).await.is_err() {
                    break;
                }
            }
            incoming = tokio::time::timeout(ACCEPT_POLL_INTERVAL, socket.recv()) => {
                match incoming {
                    Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
                    Ok(Some(Ok(_message))) => {}
                    Ok(Some(Err(_))) => break,
                    Err(_) => {}
                }
            }
        }
    }
}

async fn handle_not_implemented_connection(mut socket: WebSocket) {
    let _ = socket
        .send(Message::Text(
            json!({
                "error": "This WebSocket route is documented but not implemented yet."
            })
            .to_string()
            .into(),
        ))
        .await;
}

async fn send_connection_state(
    socket: &mut WebSocket,
    state: IRacingConnectionState,
) -> Result<(), ()> {
    let payload = serde_json::to_string(&state).map_err(|_| ())?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}
