#[cfg(not(windows))]
compile_error!("iracing-broadcast-sdk only supports Windows.");

mod client;
mod command;
mod error;
mod message_format;
mod pad_car_number;

pub use client::Client;
