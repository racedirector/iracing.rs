//! In-process server controls for the status app.
//!
//! The app exposes transport settings through Tauri commands and starts local
//! HTTP/WebSocket listeners only when the user enables them. gRPC is modeled in
//! the same settings contract, but remains a tonic placeholder until protobuf
//! services are defined.

use base64::{engine::general_purpose::STANDARD, Engine};
use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Write as _,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::State;

const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_REQUEST_BYTES: usize = 8192;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Shared state for local server settings and runtime handles.
#[derive(Debug)]
pub struct ServerManager {
    settings: Mutex<ServerSettings>,
    http: Mutex<Option<ServerHandle>>,
    websocket: Mutex<Option<ServerHandle>>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self {
            settings: Mutex::new(ServerSettings::default()),
            http: Mutex::new(None),
            websocket: Mutex::new(None),
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
    }
}

impl ServerManager {
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
            grpc: grpc_status(&lock(&self.settings)),
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
        self.reconcile_http(previous_settings, settings)?;
        self.reconcile_websocket(previous_settings, settings)?;
        Ok(())
    }

    fn reconcile_http(
        &self,
        previous_settings: &ServerSettings,
        settings: &ServerSettings,
    ) -> Result<(), String> {
        let should_run = settings.general.http_enabled;
        let did_run = previous_settings.general.http_enabled;
        let config_changed = previous_settings.http != settings.http;

        let mut handle = lock(&self.http);
        if did_run && (!should_run || config_changed) {
            stop_transport(&mut handle);
        }

        if should_run && (!did_run || config_changed || handle.is_none()) {
            let next_handle = start_http_server(settings.http.clone())?;
            *handle = Some(next_handle);
        }

        Ok(())
    }

    fn reconcile_websocket(
        &self,
        previous_settings: &ServerSettings,
        settings: &ServerSettings,
    ) -> Result<(), String> {
        let should_run = settings.general.websocket_enabled;
        let did_run = previous_settings.general.websocket_enabled;
        let config_changed = previous_settings.websocket != settings.websocket;

        let mut handle = lock(&self.websocket);
        if did_run && (!should_run || config_changed) {
            stop_transport(&mut handle);
        }

        if should_run && (!did_run || config_changed || handle.is_none()) {
            let next_handle = start_websocket_server(settings.websocket.clone())?;
            *handle = Some(next_handle);
        }

        Ok(())
    }
}

/// Complete server settings edited by the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    pub general: ServerGeneralSettings,
    pub http: TransportSettings,
    pub websocket: TransportSettings,
    pub grpc: TransportSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            general: ServerGeneralSettings::default(),
            http: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32080,
            },
            websocket: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32081,
            },
            grpc: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32082,
            },
        }
    }
}

impl ServerSettings {
    fn validate(&self) -> Result<(), String> {
        self.http.validate("HTTP")?;
        self.websocket.validate("WebSocket")?;
        self.grpc.validate("gRPC")?;
        Ok(())
    }
}

/// Feature flags for each local transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerGeneralSettings {
    pub http_enabled: bool,
    pub websocket_enabled: bool,
    pub grpc_enabled: bool,
}

impl Default for ServerGeneralSettings {
    fn default() -> Self {
        Self {
            http_enabled: false,
            websocket_enabled: false,
            grpc_enabled: false,
        }
    }
}

/// Bind settings shared by all transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSettings {
    pub host: String,
    pub port: u16,
}

impl TransportSettings {
    fn validate(&self, label: &str) -> Result<(), String> {
        if self.host.trim().is_empty() {
            return Err(format!("{label} host is required."));
        }

        if self.port == 0 {
            return Err(format!("{label} port must be between 1 and 65535."));
        }

        Ok(())
    }
}

/// Settings plus live runtime status for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerState {
    pub settings: ServerSettings,
    pub status: ServerRuntimeStatus,
}

/// Runtime status for every transport.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRuntimeStatus {
    pub http: TransportRuntimeStatus,
    pub websocket: TransportRuntimeStatus,
    pub grpc: TransportRuntimeStatus,
}

/// UI-friendly transport status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TransportRuntimeStatus {
    Disabled,
    Running { endpoint: String },
    Placeholder { message: String },
}

#[derive(Debug)]
struct ServerHandle {
    endpoint: String,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
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

fn start_http_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    let listener = bind_listener(&settings, "HTTP")?;
    let endpoint = format!("http://{}:{}", settings.host, settings.port);
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let join = thread::spawn(move || run_http_server(listener, thread_shutdown));

    Ok(ServerHandle {
        endpoint,
        shutdown,
        join: Some(join),
    })
}

fn start_websocket_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    let listener = bind_listener(&settings, "WebSocket")?;
    let endpoint = format!("ws://{}:{}", settings.host, settings.port);
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let join = thread::spawn(move || run_websocket_server(listener, thread_shutdown));

    Ok(ServerHandle {
        endpoint,
        shutdown,
        join: Some(join),
    })
}

fn bind_listener(settings: &TransportSettings, label: &str) -> Result<TcpListener, String> {
    let bind_address = format!("{}:{}", settings.host, settings.port);
    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("{label} failed to bind {bind_address}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("{label} failed to enter non-blocking mode: {error}"))?;
    Ok(listener)
}

fn run_http_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => respond_to_http_request(&mut stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => thread::sleep(ACCEPT_POLL_INTERVAL),
        }
    }
}

fn respond_to_http_request(stream: &mut TcpStream) {
    let _ = stream.set_read_timeout(Some(CONNECTION_READ_TIMEOUT));
    let mut buffer = [0_u8; MAX_REQUEST_BYTES];
    let request_len = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..request_len]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, content_type, body) = match path {
        "/" => (
            "200 OK",
            "text/plain; charset=utf-8",
            "iRacing status HTTP server\n",
        ),
        "/health" => (
            "200 OK",
            "application/json",
            "{\"status\":\"ok\",\"service\":\"iracing-status\"}\n",
        ),
        _ => ("404 Not Found", "text/plain; charset=utf-8", "Not found\n"),
    };

    let response = http_response(status, content_type, body);
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn run_websocket_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => handle_websocket_connection(&mut stream, &shutdown),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => thread::sleep(ACCEPT_POLL_INTERVAL),
        }
    }
}

fn handle_websocket_connection(stream: &mut TcpStream, shutdown: &AtomicBool) {
    let _ = stream.set_read_timeout(Some(CONNECTION_READ_TIMEOUT));
    let mut buffer = [0_u8; MAX_REQUEST_BYTES];
    let request_len = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..request_len]);

    let Some(key) = websocket_key(&request) else {
        let response = http_response(
            "400 Bad Request",
            "text/plain; charset=utf-8",
            "Missing Sec-WebSocket-Key\n",
        );
        let _ = stream.write_all(response.as_bytes());
        return;
    };

    let accept_key = websocket_accept_key(key);
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept_key}\r\n\r\n"
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    while !shutdown.load(Ordering::Acquire) {
        let mut frame_header = [0_u8; 2];
        match stream.read(&mut frame_header) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
}

fn websocket_key(request: &str) -> Option<&str> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("sec-websocket-key") {
            Some(value.trim())
        } else {
            None
        }
    })
}

fn websocket_accept_key(key: &str) -> String {
    let mut source = String::with_capacity(key.len() + WEBSOCKET_GUID.len());
    source.push_str(key);
    source.push_str(WEBSOCKET_GUID);
    STANDARD.encode(digest(&SHA1_FOR_LEGACY_USE_ONLY, source.as_bytes()).as_ref())
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    let mut response = String::new();
    let _ = write!(
        response,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {body}",
        body.len()
    );
    response
}

fn stop_transport(handle: &mut Option<ServerHandle>) {
    if let Some(mut handle) = handle.take() {
        handle.shutdown.store(true, Ordering::Release);
        if let Some(join) = handle.join.take() {
            let _ = join.join();
        }
    }
}

fn transport_status(handle: &Option<ServerHandle>) -> TransportRuntimeStatus {
    match handle {
        Some(handle) => TransportRuntimeStatus::Running {
            endpoint: handle.endpoint.clone(),
        },
        None => TransportRuntimeStatus::Disabled,
    }
}

fn grpc_status(settings: &ServerSettings) -> TransportRuntimeStatus {
    if settings.general.grpc_enabled {
        TransportRuntimeStatus::Placeholder {
            message: "tonic gRPC server placeholder; protobuf services are not defined yet."
                .to_string(),
        }
    } else {
        TransportRuntimeStatus::Disabled
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_keep_all_transports_disabled() {
        let settings = ServerSettings::default();

        assert!(!settings.general.http_enabled);
        assert!(!settings.general.websocket_enabled);
        assert!(!settings.general.grpc_enabled);
        assert_eq!(settings.http.port, 32080);
        assert_eq!(settings.websocket.port, 32081);
        assert_eq!(settings.grpc.port, 32082);
    }

    #[test]
    fn transport_settings_require_a_host_and_non_zero_port() {
        let missing_host = TransportSettings {
            host: String::new(),
            port: 32080,
        };
        let missing_port = TransportSettings {
            host: "127.0.0.1".to_string(),
            port: 0,
        };

        assert!(missing_host.validate("HTTP").is_err());
        assert!(missing_port.validate("HTTP").is_err());
    }

    #[test]
    fn websocket_accept_key_matches_rfc_example() {
        let accept_key = websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ==");

        assert_eq!(accept_key, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
