//! Native iRacing SDK wire definitions.
//!
//! This module re-exports the standalone [`iracing_irsdk`] crate so existing
//! `iracing_sdk::irsdk` imports remain source-compatible.

pub use iracing_irsdk::*;

mod telemetry_integration;
