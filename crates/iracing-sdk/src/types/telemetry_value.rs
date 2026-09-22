use serde::{Deserialize, Serialize};

use crate::{BitField, IRacingSDKError, Result, VarData, VariableInfo, irsdk::VariableType};

/// Runtime value type that can hold any telemetry data.
///
/// SDK decoding produces only `Char`, `Bool`, `Int32`, `BitField`, `Float32`,
/// `Float64`, and arrays. Other integer variants remain available for callers
/// constructing values directly or reading previously serialized values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TelemetryValue {
    /// An 8-bit character value (`irsdk_char`).
    Char(u8),
    /// An 8-bit signed integer.
    Int8(i8),
    /// An 8-bit unsigned integer.
    UInt8(u8),
    /// A 16-bit signed integer.
    Int16(i16),
    /// A 16-bit unsigned integer.
    UInt16(u16),
    /// A 32-bit signed integer (`irsdk_int`).
    Int32(i32),
    /// A 32-bit unsigned integer.
    UInt32(u32),
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

impl TelemetryValue {
    /// Decodes the variable described by `info` from a complete telemetry frame.
    ///
    /// `info.offset` is relative to the start of `data`; a zero element count
    /// produces an empty [`Self::Array`].
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata does not describe an SDK storage type,
    /// its extent overflows, or the requested bytes are outside `data`.
    pub fn decode(data: &[u8], info: &VariableInfo) -> Result<Self> {
        let bytes = info.bytes_in(data)?;
        match info.count {
            0 => Ok(Self::Array(Vec::new())),
            1 => Self::decode_scalar(bytes, info.data_type),
            _ => Self::decode_array(bytes, info.data_type),
        }
    }

    fn decode_scalar(bytes: &[u8], data_type: VariableType) -> Result<Self> {
        match data_type {
            VariableType::Character => {
                <u8 as VarData>::decode_value(bytes, data_type).map(Self::Char)
            }
            VariableType::BitField => {
                <BitField as VarData>::decode_value(bytes, data_type).map(Self::BitField)
            }
            VariableType::Boolean => {
                <bool as VarData>::decode_value(bytes, data_type).map(Self::Bool)
            }
            VariableType::Integer => {
                <i32 as VarData>::decode_value(bytes, data_type).map(Self::Int32)
            }
            VariableType::Float => {
                <f32 as VarData>::decode_value(bytes, data_type).map(Self::Float32)
            }
            VariableType::Double => {
                <f64 as VarData>::decode_value(bytes, data_type).map(Self::Float64)
            }
            VariableType::ElementTypeCount => Err(IRacingSDKError::parse_error(
                "TelemetryValue::decode",
                "ElementTypeCount is not a storage type",
            )),
        }
    }

    fn decode_array(bytes: &[u8], data_type: VariableType) -> Result<Self> {
        let element_size = data_type.byte_size().ok_or_else(|| {
            IRacingSDKError::parse_error(
                "TelemetryValue::decode_array",
                "invalid telemetry storage type",
            )
        })?;
        let mut values = Vec::with_capacity(bytes.len() / element_size);
        for chunk in bytes.chunks_exact(element_size) {
            values.push(Self::decode_scalar(chunk, data_type)?);
        }

        Ok(Self::Array(values))
    }
}

/// Decodes telemetry values using their variable metadata.
///
/// Implementors provide access to the raw data for a telemetry frame while
/// callers supply the corresponding [`VariableInfo`].
pub trait TelemetryValueProvider {
    /// Decodes the telemetry value described by `info`.
    fn telemetry_value(&self, info: &VariableInfo) -> Result<TelemetryValue>;
}
