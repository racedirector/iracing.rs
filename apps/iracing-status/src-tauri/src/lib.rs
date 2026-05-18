//! Tauri application bootstrap for the iRacing status example.
//!
//! The frontend is a React status panel. The backend exposes two commands from
//! [`state`]:
//!
//! - `get_connection_state` for one-shot snapshots.
//! - `observe_connection_state` for event-driven updates.
//!
//! The observer state is registered with `Builder::manage` so command handlers
//! can coordinate one background monitor for the app process.

mod broadcast_client;
mod server;
mod state;

/// Configure and run the Tauri application.
///
/// This is separated from `main.rs` by the default Tauri template so the app can
/// be built as a library for desktop and mobile entry points. The important
/// pieces for this example are:
///
/// - `.manage(state::ConnectionStateObserver::default())` installs shared
///   backend state that commands can borrow with `State<_>`.
/// - `.invoke_handler(...)` exposes Rust functions to the JavaScript
///   `invoke(...)` API.
/// - `tauri_plugin_opener` remains from the scaffold and is unrelated to the
///   connection-state observer.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::ConnectionStateObserver::default())
        .manage(server::ServerManager::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            state::get_connection_state,
            state::observe_connection_state,
            broadcast_client::send_broadcast_client_request,
            server::get_server_state,
            server::set_server_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
