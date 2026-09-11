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
        info.storage_byte_size()?;
        match info.count {
            0 => Ok(Self::Array(Vec::new())),
            1 => Self::decode_scalar(data, info),
            _ => Self::decode_array(data, info),
        }
    }

    fn decode_scalar(data: &[u8], info: &VariableInfo) -> Result<Self> {
        match info.data_type {
            VariableType::Character => u8::from_bytes(data, info).map(Self::Char),
            VariableType::BitField => BitField::from_bytes(data, info).map(Self::BitField),
            VariableType::Boolean => bool::from_bytes(data, info).map(Self::Bool),
            VariableType::Integer => i32::from_bytes(data, info).map(Self::Int32),
            VariableType::Float => f32::from_bytes(data, info).map(Self::Float32),
            VariableType::Double => f64::from_bytes(data, info).map(Self::Float64),
            VariableType::ElementTypeCount => Err(IRacingSDKError::parse_error(
                "TelemetryValue::decode",
                "ElementTypeCount is not a storage type",
            )),
        }
    }

    fn decode_array(data: &[u8], info: &VariableInfo) -> Result<Self> {
        let element_size = info.storage_byte_size()?;
        let mut values = Vec::with_capacity(info.count);
        let mut element_info = info.clone();
        element_info.count = 1;

        for index in 0..info.count {
            let offset_delta = index
                .checked_mul(element_size)
                .ok_or_else(|| IRacingSDKError::memory_access_error(info.offset))?;

            element_info.offset = info
                .offset
                .checked_add(offset_delta)
                .ok_or_else(|| IRacingSDKError::memory_access_error(info.offset))?;

            values.push(Self::decode_scalar(data, &element_info)?);
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
