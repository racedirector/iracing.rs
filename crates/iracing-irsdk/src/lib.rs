#![warn(missing_docs)]
//! Rust representations of the native iRacing SDK wire contract.
//!
//! This crate owns types that can be understood from `irsdk_defines.h` and the
//! documented byte layout alone: fixed-layout headers, variable metadata,
//! primitive storage types, constants, enums, flags, and broadcast commands.
//! It deliberately does not parse IBT files, access Windows shared memory,
//! construct telemetry schemas, decode session YAML, or run telemetry streams.

mod bitfield;
mod macros;
mod parse_utils;

#[cfg(feature = "broadcast")]
pub mod broadcast;

pub mod constants;
pub mod flags;
pub mod telemetry;
pub mod variable_type;

mod disk_sub_header;
mod error;
mod header;
mod ibt_header;
mod variable_buffer;
mod variable_header;

pub use bitfield::BitField;
#[cfg(feature = "broadcast")]
pub use broadcast::*;
pub use disk_sub_header::DiskSubHeader;
pub use error::{Error, Result};
pub use flags::*;
pub use header::Header;
pub use ibt_header::IbtHeader;
pub use parse_utils::{decode, encode};
pub use telemetry::*;
pub use variable_buffer::VariableBuffer;
pub use variable_header::VariableHeader;
pub use variable_type::VariableType;
