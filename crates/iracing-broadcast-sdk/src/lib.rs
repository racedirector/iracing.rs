#[cfg(windows)]
mod client;
mod command;
mod error;
mod message_format;
mod pad_car_number;

#[cfg(windows)]
pub use client::Client;
pub use command::Command;

pub use iracing_irsdk::{CameraState, broadcast::*};
