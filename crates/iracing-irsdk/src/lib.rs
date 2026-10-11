#![warn(missing_docs)]
//! Rust representations of the native iRacing SDK wire contract.
//!
//! This crate owns types that can be understood from `irsdk_defines.h` and the
//! documented byte layout alone: fixed-layout headers, variable metadata,
//! primitive storage types, constants, enums, flags, and broadcast commands.
//! It deliberately does not parse IBT files, access Windows shared memory,
//! construct telemetry schemas, decode session YAML, or run telemetry streams.
//!
//! # Representation and interpretation
//!
//! Native-layout decoding copies an exact-size value into owned storage from
//! any input alignment. The SDK representation is little-endian: these copies
//! require a little-endian target and do not swap bytes. Fixed-layout types keep
//! compile-time ABI assertions and layout/round-trip tests beside their
//! definitions; those checks own sizes, alignments, and offsets.
//!
//! `FromBytes` permits every bit pattern of that Rust type, not every meaningful
//! SDK field value. [`VariableHeader`] checks its [`VariableType`] discriminant.
//! Neither establishes source bounds, nonnegative geometry, valid frame extents,
//! or a coherent live snapshot. Source/layout validation belongs to consumers.
//!
//! [`VariableType::BitField`] describes four-byte little-endian storage, distinct
//! from a generic Rust wrapper or named flag family. [`flags`] owns typed masks
//! and packed-field interpretation. The existing [`BitField`] API remains on its
//! symbol; migration is tracked in
//! [issue #167](https://github.com/racedirector/iracing.rs/issues/167).
//!
//! Native SDK names identify upstream provenance. Repository-enforced behavior
//! is the Rust definition, conversion, and adjacent tests; captured schemas do
//! not define this crate's ABI.

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
