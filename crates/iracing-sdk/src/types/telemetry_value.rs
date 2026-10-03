use serde::{Deserialize, Serialize};

use crate::{FieldLayout, Result};

/// Runtime value type that can hold any telemetry data.
///
/// SDK decoding produces only `Char`, `Bool`, `Int32`, `BitField`, `Float32`,
/// `Float64`, and arrays. Other integer variants remain available for callers
/// constructing values directly or reading previously serialized values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TelemetryValue {
    /// An 8-bit character value (`irsdk_char`).
    Char(u8),
    /// A 32-bit signed integer (`irsdk_int`).
    Int32(i32),
    /// A 32-bit IEEE 754 floating-point value (`irsdk_float`).
    Float32(f32),
    /// A 64-bit IEEE 754 floating-point value (`irsdk_double`).
    Float64(f64),
    /// A boolean value (`irsdk_bool`).
    Bool(bool),
    /// A 32-bit bitfield (`irsdk_bitField`).
    BitField(super::BitField),
    /// An array of homogeneous telemetry values (multi-element variables).
    Array(Vec<TelemetryValue>),
}

/// Decodes telemetry values using their variable metadata.
///
/// Implementors provide access to the raw data for a telemetry frame while
/// callers supply the corresponding [`FieldLayout`].
pub trait TelemetryValueProvider {
    /// Decodes the telemetry value described by `info`.
    fn telemetry_value(&self, info: &FieldLayout) -> Result<TelemetryValue>;
}
