//! Binary entry point for the desktop Tauri application.
//!
//! The application logic lives in `iracing_status_lib::run` so Tauri can share
//! setup code across supported entry points. This file keeps only the
//! executable-specific Windows subsystem attribute and then delegates to the
//! library.

// Prevents an extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    iracing_status_lib::run()
}
