//! Typed commands and native dispatch for iRacing's broadcast channel.
//!
//! [`Command`] owns operation-level inputs and their conversion into
//! [`BroadcastMessage`]. Encoding is portable and can be tested without a
//! simulator. Raw message identifiers, argument words, and domain enums are
//! owned by [`iracing_irsdk`] and re-exported here for command construction.
//!
//! On Windows, `Client` registers the native broadcast message and sends encoded
//! commands through Win32. Registration does not establish simulator readiness,
//! and successful dispatch does not acknowledge that the simulator applied an
//! operation. This crate does not acquire telemetry or resolve session selectors;
//! callers own those prerequisites.
//!
//! Command conversion and its tests own validation and packing behavior. The
//! CLI crate owns operator-facing parsing and dispatch composition.

#[cfg(windows)]
mod client;
mod command;
pub mod error;
mod pad_car_number;

#[cfg(windows)]
pub use client::Client;
pub use command::Command;

pub use iracing_irsdk::{CameraState, broadcast::*};
