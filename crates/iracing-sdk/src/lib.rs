#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![warn(missing_docs)]
//! Low-level iRacing telemetry parsing primitives.
//!
//! This crate is designed as a foundation for tools that consume iRacing data from:
//!
//! - Recorded telemetry files (`.ibt`) on any platform
//! - Live shared memory on Windows
//!
//! # API map
//!
//! - Replay/offline path (cross-platform):
//!   - [`reader::ibt::IbtReader`]
//!   - [`types::VariableSchema`], [`types::VarData`]
//! - Streaming adapter path:
//!   - [`FramePacket`], [`provider::Provider`], [`providers::ibt::IbtProvider`], [`DynamicFrame`]
//!   - [`FrameAdapter`], [`AdapterValidation`], [`FieldExtraction`], [`SchemaProvider`]
//! - Source acquisition primitives:
//!   - [`reader`] for checked random access, header-directed snapshots, and
//!     borrowed mapped-memory access
//! - Session data path:
//!   - [`schema::SessionInfo`], [`schema::SessionInfoParser`]
//!   - [`yaml_utils`] for iRacing YAML cleanup
//! - Live path (Windows only):
//!   - `LiveProvider`, `WindowsConnection`, `WaitResult`
//!   - `Broadcast`, `BroadcastCommand`
//!   - [`PitCommand`] is cross-platform typed data for pit-service broadcast commands.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use iracing_sdk::{VarData, VariableSchema, reader::ibt::IbtReader};
//!
//! fn main() -> iracing_sdk::Result<()> {
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     let variable_headers = reader
//!         .variable_headers_buffer()?
//!         .ok_or_else(|| iracing_sdk::IRacingSDKError::parse_error(
//!             "schema lookup",
//!             "recording does not contain variable headers",
//!         ))?;
//!     let schema = VariableSchema::from_variable_headers(
//!         variable_headers,
//!         reader.recording().frame_length(),
//!     )?;
//!     let speed_info = schema
//!         .get_variable("Speed")
//!         .ok_or_else(|| iracing_sdk::IRacingSDKError::parse_error(
//!             "schema lookup",
//!             "missing Speed variable",
//!         ))?
//!         .clone();
//!
//!     while let Some(frame) = reader.read_next_frame()? {
//!         let frame: Vec<u8> = frame.into_buffer().into();
//!         let speed_mps = f32::from_bytes(&frame, &speed_info)?;
//!         let _speed_kph = speed_mps * 3.6;
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Feature flags
//!
//! - `codegen`: enables schema generation helpers like `session_root_schema`.
//! - `derive`: re-exports telemetry adapter derive macros from `iracing-sdk-derive`.
//!   Generated derive code uses `iracing-sdk`'s internal `tracing` re-export, so downstream
//!   crates do not need a direct `tracing` dependency just to compile derived adapters.
//! - `schema-discovery`: enables unknown-field discovery overlays for session schemas.
//! - `benchmark`: enables benchmark targets.
//!
pub mod adapters;
mod error;
mod parse_utils;
pub mod reader;
pub mod test_utils;
pub mod types;
pub mod yaml_utils;

// Stream-based modules
pub mod connections;
pub mod provider;
pub mod providers;
pub mod stream;
pub mod telemetry;

#[cfg(feature = "benchmark")]
#[doc(hidden)]
pub mod benchmarking;

// Data model modules
pub mod schema;

// Core exports
pub use adapters::*;
pub use error::*;
pub use types::*;

#[doc(hidden)]
pub mod __private {
    pub use tracing;
}

#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use iracing_sdk_derive::*;

// Platform-specific modules
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub mod windows;

// Windows memory exports
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use windows::{Broadcast, BroadcastCommand, Connection as WindowsConnection, WaitResult};

// Main API exports
pub use connections::ibt::IbtConnection;
pub use connections::live::LiveConnection;
pub use types::UpdateRate;
