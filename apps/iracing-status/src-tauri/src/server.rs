//! In-process server controls for the status app.
//!
//! The app exposes transport settings through Tauri commands and starts local
//! HTTP/WebSocket/gRPC listeners only when the user enables them.

mod grpc;
mod http;
mod settings;
mod transport;
mod websocket;

use std::sync::{Mutex, MutexGuard};
use tauri::State;

use grpc::start_grpc_server;
use http::start_http_server;
use settings::{ServerRuntimeStatus, TransportRuntimeStatus};
pub use settings::{ServerSettings, ServerState};
use transport::{stop_transport, transport_status, ServerHandle};
use websocket::start_websocket_server;

/// Shared state for local server settings and runtime handles.
#[derive(Debug)]
pub struct ServerManager {
    settings: Mutex<ServerSettings>,
    http: Mutex<Option<ServerHandle>>,
    websocket: Mutex<Option<ServerHandle>>,
    grpc: Mutex<Option<ServerHandle>>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self {
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
            || start_http_server(settings.http.clone()),
        )?;
        self.reconcile_transport(
            previous_settings.general.websocket_enabled,
            settings.general.websocket_enabled,
            previous_settings.websocket != settings.websocket,
            &self.websocket,
            || start_websocket_server(settings.websocket.clone()),
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
