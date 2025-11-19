#![deny(clippy::all)]

pub mod fps;
pub mod replay;
pub mod session;
pub mod simulation;
pub mod states;
pub mod track_surface;

#[cfg(all(target_os = "windows", feature = "telemetry"))]
pub mod telemetry;
