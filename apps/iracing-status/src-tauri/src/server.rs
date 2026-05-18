//! In-process server controls for the status app.
//!
//! The app exposes transport settings through Tauri commands and starts local
//! HTTP/WebSocket/gRPC listeners only when the user enables them.

mod grpc;
mod http;
mod settings;
mod transport;
mod websocket;

use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, State};

use grpc::start_grpc_server;
use http::start_http_server;
use settings::{ServerRuntimeStatus, TransportRuntimeStatus};
pub use settings::{ServerSettings, ServerState};
use transport::{stop_transport, transport_status, ServerHandle};
use websocket::{start_websocket_server, WebsocketRuntime};

use crate::state::ConnectionStateObserver;

/// Shared state for local server settings and runtime handles.
#[derive(Debug)]
pub struct ServerManager {
    connection_state_observer: Arc<ConnectionStateObserver>,
    websocket_runtime: Option<WebsocketRuntime>,
    settings: Mutex<ServerSettings>,
    http: Mutex<Option<ServerHandle>>,
    websocket: Mutex<Option<ServerHandle>>,
    grpc: Mutex<Option<ServerHandle>>,
}

impl Default for ServerManager {
    fn default() -> Self {
        let observer = Arc::new(ConnectionStateObserver::default());
        Self {
            connection_state_observer: observer,
            websocket_runtime: None,
            settings: Mutex::new(ServerSettings::default()),
            http: Mutex::new(None),
            websocket: Mutex::new(None),
            grpc: Mutex::new(None),
        }
    }
}

impl ServerManager {
    pub fn new(app: AppHandle, observer: Arc<ConnectionStateObserver>) -> Self {
        Self {
            connection_state_observer: Arc::clone(&observer),
            websocket_runtime: Some(WebsocketRuntime::new(app, observer)),
            settings: Mutex::new(ServerSettings::default()),
            http: Mutex::new(None),
            websocket: Mutex::new(None),
            grpc: Mutex::new(None),
        }
    }
}

impl Drop for ServerManager {
    fn drop(&mut self) {
        if let Ok(mut handle) = self.http.lock() {
            stop_transport(&mut handle);
        }

        if let Ok(mut handle) = self.websocket.lock() {
            stop_transport(&mut handle);
        }

        if let Ok(mut handle) = self.grpc.lock() {
            stop_transport(&mut handle);
        }
    }
}

impl ServerManager {
    pub(crate) fn grpc_endpoint(&self) -> Result<String, String> {
        match self.current_status().grpc {
            TransportRuntimeStatus::Running { endpoint } => Ok(endpoint),
            TransportRuntimeStatus::Disabled => Err("gRPC service is not running.".to_string()),
        }
    }

    fn current_state(&self) -> ServerState {
        let settings = lock(&self.settings).clone();
        ServerState {
            settings,
            status: self.current_status(),
        }
    }

    fn current_status(&self) -> ServerRuntimeStatus {
        ServerRuntimeStatus {
            http: transport_status(&lock(&self.http)),
            websocket: transport_status(&lock(&self.websocket)),
            grpc: transport_status(&lock(&self.grpc)),
        }
    }

    fn apply_settings(&self, settings: ServerSettings) -> Result<ServerState, String> {
        settings.validate()?;

        let previous_settings = lock(&self.settings).clone();

        if let Err(error) = self.reconcile_settings(&previous_settings, &settings) {
            let _ = self.reconcile_settings(&settings, &previous_settings);
            return Err(error);
        }

        *lock(&self.settings) = settings;

        Ok(self.current_state())
    }

    fn reconcile_settings(
        &self,
        previous_settings: &ServerSettings,
        settings: &ServerSettings,
    ) -> Result<(), String> {
        self.reconcile_transport(
            previous_settings.general.http_enabled,
            settings.general.http_enabled,
            previous_settings.http != settings.http,
            &self.http,
            || {
                start_http_server(
                    settings.http.clone(),
                    Arc::clone(&self.connection_state_observer),
                )
            },
        )?;
        self.reconcile_transport(
            previous_settings.general.websocket_enabled,
            settings.general.websocket_enabled,
            previous_settings.websocket != settings.websocket,
            &self.websocket,
            || start_websocket_server(settings.websocket.clone(), self.websocket_runtime.clone()),
        )?;
        self.reconcile_transport(
            previous_settings.general.grpc_enabled,
            settings.general.grpc_enabled,
            previous_settings.grpc != settings.grpc,
            &self.grpc,
            || start_grpc_server(settings.grpc.clone()),
        )?;

        Ok(())
    }

    fn reconcile_transport(
        &self,
        did_run: bool,
        should_run: bool,
        config_changed: bool,
        handle: &Mutex<Option<ServerHandle>>,
        start: impl FnOnce() -> Result<ServerHandle, String>,
    ) -> Result<(), String> {
        let mut handle = lock(handle);
        if did_run && (!should_run || config_changed) {
            stop_transport(&mut handle);
        }

        if should_run && (!did_run || config_changed || handle.is_none()) {
            *handle = Some(start()?);
        }

        Ok(())
    }
}

/// Return current server settings and runtime status.
#[tauri::command]
pub fn get_server_state(manager: State<'_, ServerManager>) -> ServerState {
    manager.current_state()
}

/// Replace server settings and immediately apply transport lifecycle changes.
#[tauri::command]
pub fn set_server_settings(
    manager: State<'_, ServerManager>,
    settings: ServerSettings,
) -> Result<ServerState, String> {
    manager.apply_settings(settings)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::{Duration, Instant},
    };

    const EVENTUALLY_TIMEOUT: Duration = Duration::from_secs(5);
    const EVENTUALLY_POLL_INTERVAL: Duration = Duration::from_millis(25);

    #[test]
    fn http_transport_can_start_rebind_and_stop() {
        let manager = ServerManager::default();
        let first_port = free_port();
        let second_port = free_port_except(first_port);

        let mut settings = ServerSettings::default();
        settings.general.http_enabled = true;
        settings.http.port = first_port;

        let state = manager.apply_settings(settings.clone()).unwrap();
        assert_running_endpoint(state.status.http, format!("http://127.0.0.1:{first_port}"));

        let response = eventually(|| get_http_health(first_port));
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains(r#""status":"ok""#), "{response}");
        assert!(
            response.contains(r#""service":"iracing-status""#),
            "{response}"
        );
        let response = eventually(|| get_http_status(first_port));
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains(r#""process":"checking""#), "{response}");
        assert!(response.contains(r#""sim":"disconnected""#), "{response}");
        assert!(response.contains(r#""telemetry":"disconnected""#), "{response}");

        settings.http.port = second_port;
        let state = manager.apply_settings(settings).unwrap();
        assert_running_endpoint(state.status.http, format!("http://127.0.0.1:{second_port}"));

        assert_port_closed(first_port);
        let response = eventually(|| get_http_health(second_port));
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");

        let state = manager.apply_settings(ServerSettings::default()).unwrap();
        assert!(matches!(
            state.status.http,
            TransportRuntimeStatus::Disabled
        ));
        assert_port_closed(second_port);
    }

    #[test]
    fn websocket_transport_can_start_accept_connections_and_stop() {
        let manager = ServerManager::default();
        let port = free_port();

        let mut settings = ServerSettings::default();
        settings.general.websocket_enabled = true;
        settings.websocket.port = port;

        let state = manager.apply_settings(settings).unwrap();
        assert_running_endpoint(state.status.websocket, format!("ws://127.0.0.1:{port}"));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            eventually_async(|| async {
                tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
            .await;
        });

        let state = manager.apply_settings(ServerSettings::default()).unwrap();
        assert!(matches!(
            state.status.websocket,
            TransportRuntimeStatus::Disabled
        ));
        assert_port_closed(port);
    }

    #[cfg(windows)]
    #[test]
    fn grpc_transport_can_start_accept_reflection_clients_and_stop() {
        let manager = ServerManager::default();
        let port = free_port();

        let mut settings = ServerSettings::default();
        settings.general.grpc_enabled = true;
        settings.grpc.port = port;

        let state = manager.apply_settings(settings).unwrap();
        assert_running_endpoint(state.status.grpc, format!("http://127.0.0.1:{port}"));

        let endpoint = format!("http://127.0.0.1:{port}");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let mut reflection_client = eventually_async(|| async {
                let channel = tonic::transport::Endpoint::new(endpoint.clone())
                    .map_err(|error| error.to_string())?
                    .connect()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(tonic_reflection::pb::v1alpha::server_reflection_client::ServerReflectionClient::new(channel))
            })
            .await;

            let reflection_request = tonic::Request::new(tokio_stream::once(
                tonic_reflection::pb::v1alpha::ServerReflectionRequest {
                    host: String::new(),
                    message_request: Some(
                        tonic_reflection::pb::v1alpha::server_reflection_request::MessageRequest::ListServices(
                            String::new(),
                        ),
                    ),
                },
            ));
            let mut reflection_responses = reflection_client
                .server_reflection_info(reflection_request)
                .await
                .expect("reflection should be callable")
                .into_inner();
            let response = reflection_responses
                .message()
                .await
                .expect("reflection response should arrive")
                .expect("reflection stream should yield one response");
            let services = match response.message_response.expect("reflection payload should exist")
            {
                tonic_reflection::pb::v1alpha::server_reflection_response::MessageResponse::ListServicesResponse(
                    services,
                ) => services.service,
                other => panic!("unexpected reflection response: {other:?}"),
            };
            assert!(
                services.iter().any(|service| {
                    service.name == "iracing.broadcast.Broadcast"
                }),
                "registered services were: {:?}",
                services
                    .iter()
                    .map(|service| service.name.clone())
                    .collect::<Vec<_>>()
            );

            let mut client = eventually_async(|| async {
                iracing_broadcast_grpc_service::RawBroadcastClient::connect(endpoint.clone())
                    .await
                    .map_err(|error| error.to_string())
            })
            .await;

            let response = client
                .get_available_cameras(())
                .await
                .expect("available cameras should be callable")
                .into_inner();
            assert!(response.camera_groups.is_empty());
            assert_eq!(response.car_index, 0);
            assert_eq!(response.group, 0);
            assert_eq!(response.camera, 0);
        });

        let state = manager.apply_settings(ServerSettings::default()).unwrap();
        assert!(matches!(
            state.status.grpc,
            TransportRuntimeStatus::Disabled
        ));
        assert_port_closed(port);
    }

    #[cfg(not(windows))]
    #[test]
    fn grpc_transport_reports_unsupported_platform_without_changing_settings() {
        let manager = ServerManager::default();
        let mut settings = ServerSettings::default();
        settings.general.grpc_enabled = true;
        settings.grpc.port = free_port();

        let error = manager
            .apply_settings(settings)
            .expect_err("non-Windows gRPC startup should fail");
        assert_eq!(error, "gRPC broadcast server requires Windows.");

        let state = manager.current_state();
        assert!(!state.settings.general.grpc_enabled);
        assert!(matches!(
            state.status.grpc,
            TransportRuntimeStatus::Disabled
        ));
    }

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn free_port_except(excluded_port: u16) -> u16 {
        loop {
            let port = free_port();
            if port != excluded_port {
                return port;
            }
        }
    }

    fn get_http_health(port: u16) -> Result<String, String> {
        get_http_response(port, "/health")
    }

    fn get_http_status(port: u16) -> Result<String, String> {
        get_http_response(port, "/status")
    }

    fn get_http_response(port: u16, path: &str) -> Result<String, String> {
        let mut stream =
            TcpStream::connect(("127.0.0.1", port)).map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .map_err(|error| error.to_string())?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;
        Ok(response)
    }

    fn assert_running_endpoint(status: TransportRuntimeStatus, expected_endpoint: String) {
        match status {
            TransportRuntimeStatus::Running { endpoint } => assert_eq!(endpoint, expected_endpoint),
            TransportRuntimeStatus::Disabled => panic!("transport should be running"),
        }
    }

    fn assert_port_closed(port: u16) {
        eventually(|| match TcpStream::connect(("127.0.0.1", port)) {
            Ok(_) => Err(format!("port {port} is still accepting connections")),
            Err(_) => Ok(()),
        });
    }

    fn eventually<T>(mut action: impl FnMut() -> Result<T, String>) -> T {
        let deadline = Instant::now() + EVENTUALLY_TIMEOUT;

        loop {
            let error = match action() {
                Ok(value) => return value,
                Err(error) => error,
            };

            if Instant::now() >= deadline {
                panic!("condition was not met before timeout: {error}");
            }

            thread::sleep(EVENTUALLY_POLL_INTERVAL);
        }
    }

    async fn eventually_async<T, F, Fut>(mut action: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let deadline = Instant::now() + EVENTUALLY_TIMEOUT;

        loop {
            let error = match action().await {
                Ok(value) => return value,
                Err(error) => error,
            };

            if Instant::now() >= deadline {
                panic!("condition was not met before timeout: {error}");
            }

            tokio::time::sleep(EVENTUALLY_POLL_INTERVAL).await;
        }
    }
}
