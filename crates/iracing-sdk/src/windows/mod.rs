//! iRacing shared memory access
//!
//! This module provides direct access to iRacing's shared memory telemetry
//! following the same patterns as the official C++ SDK. The implementation
//! keeps Win32 ownership separate from the validated concrete live reader.
//!
//! # Design Philosophy
//!
//! - **Validated Mapping Generations**: Parse static layout once for each
//!   opened mapping and reconstruct the reader when the mapping changes
//! - **C++ SDK Alignment**: Use identical struct layouts and logic patterns
//!   to the official iRacing C++ SDK
//! - **Buffer Rotation**: Properly handle iRacing's 4-buffer rotation system
//!   using tick count comparison
//! - **Minimal API Surface**: Expose only what's needed for telemetry reading
//!
//! # Usage
//!
//! ```rust,ignore
//! use iracing_sdk::windows::{Connection, WaitResult};
//! use std::time::Duration;
//!
//! // Connect to iRacing
//! let mut connection = Connection::try_connect()?;
//!
//! // Wait for telemetry updates
//! match connection.wait_for_update(Duration::from_millis(100))? {
//!     WaitResult::Signaled => {
//!         if let Some(snapshot) = connection.next_frame()? {
//!             let data = snapshot.into_buffer();
//!             // Process owned telemetry data
//!         }
//!     }
//!     WaitResult::Timeout => {
//!         // No new data available
//!     }
//! }
//! ```

mod broadcast;
mod connection;
mod utils;

pub use crate::PitCommand;
pub use broadcast::{Broadcast, BroadcastCommand};
pub use connection::{Connection, WaitResult};
pub use utils::wide_string;
