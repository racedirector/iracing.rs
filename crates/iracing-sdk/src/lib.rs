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
//!   - [`IbtReader`]
//!   - [`types::VariableSchema`], [`types::VarData`]
//! - Session data path:
//!   - [`SessionInfo`], [`SessionInfoParser`]
//!   - [`yaml_utils`] for iRacing YAML cleanup
//! - Live path (Windows only):
//!   - `WindowsConnection` and `WaitResult`
//!
//! # Quick start
//!
//! ```rust,no_run
//! use iracing_sdk::{IbtReader, VarData};
//!
//! fn main() -> iracing_sdk::Result<()> {
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     let speed_info = reader
//!         .variables()
//!         .get_variable("Speed")
//!         .ok_or_else(|| iracing_sdk::IRacingSDKError::Parse {
//!             context: "schema lookup".to_string(),
//!             details: "missing Speed variable".to_string(),
//!         })?
//!         .clone();
//!
//!     while let Some((frame, _tick, _session_version)) = reader.read_next_frame()? {
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
//! - `schema-discovery`: enables unknown-field discovery overlays for session schemas.
//! - `tokio`: enables async waiting for live telemetry updates on Windows.
//! - `benchmark`: enables benchmark targets.
//!
mod error;
pub mod ibt;
pub mod schema;
pub mod types;
pub mod yaml_utils;

pub use error::*;
pub use ibt::{IbtReader, IndexedIbt, LapFlags, LapFrameIter, LapIndex, LapIndexVars, LapRef};
pub use schema::{SessionInfo, SessionInfoParser};
pub use types::*;

// Platform-specific modules
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub mod windows;

// Windows memory exports
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use windows::{Broadcast, Connection as WindowsConnection, WaitResult};
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use windows::{BroadcastCommand, PitCommand};
