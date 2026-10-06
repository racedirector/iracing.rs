#[cfg(not(windows))]
compile_error!("iracing-broadcast-sdk only supports Windows.");

mod client;
mod command;
mod error;
mod message_format;
mod pad_car_number;

pub use client::Client;
pub use command::Command;
pub use iracing_irsdk::broadcast::*;

/** Public re-exports from iracing_irsdk */
pub use iracing_irsdk::CameraState;
