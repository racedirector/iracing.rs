//! iRacing shared memory access
//!
//! This module provides direct access to iRacing's shared memory telemetry
//! following the same patterns as the official C++ SDK. The implementation
//! focuses on simplicity and performance over abstraction layers.
//!
//! # Design Philosophy
//!
//! - **Direct Memory Access**: Map iRacing's shared memory directly without
//!   unnecessary validation or abstraction layers
//! - **C++ SDK Alignment**: Use identical struct layouts and logic patterns
//!   to the official iRacing C++ SDK
//! - **Buffer Rotation**: Properly handle iRacing's 4-buffer rotation system
//!   using tick count comparison
//! - **Minimal API Surface**: Expose only what's needed for telemetry reading
//!
//! # Usage
//!
//! ```rust,ignore
//! use pitwall::windows::Connection;
//! use std::time::Duration;
//!
//! // Connect to iRacing
//! let mut connection = Connection::try_connect()?;
//!
//! // Wait for telemetry updates
//! match connection.wait_for_update(Duration::from_millis(100))? {
//!     WaitResult::Signaled => {
//!         if let Some(data) = connection.get_new_data() {
//!             // Process telemetry data
//!         }
//!     }
//!     WaitResult::Timeout => {
//!         // No new data available
//!     }
//! }
//! ```

mod connection;

pub use connection::{Connection, IRSDKHeader, VarBuf, WaitResult};
