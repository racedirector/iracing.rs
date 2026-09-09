//! Rust representations of definitions from `irsdk_defines.h`.
//!
//! The SDK declarations are modeled here independently of the crate's existing
//! domain-facing telemetry and broadcast types.

mod macros;

pub mod broadcast;
pub mod constants;
pub mod flags;
pub mod telemetry;
pub mod variable_type;

// Module API
mod disk_sub_header;
mod error;
mod header;
mod variable_buffer;
mod variable_header;
mod wire_type;

// Wire-format API
pub use disk_sub_header::DiskSubHeader;
pub use header::Header;
pub use variable_buffer::VariableBuffer;
pub use variable_header::VariableHeader;
pub use wire_type::WireType;

// SDK definition API
pub use broadcast::*;
pub use flags::*;
pub use telemetry::*;
pub use variable_type::VariableType;
