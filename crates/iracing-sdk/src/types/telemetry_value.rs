//! Runtime telemetry value decoding.

use serde::{Deserialize, Serialize};

use crate::{BitField, IRacingSDKError, VarData, VariableInfo, VariableType};

/// Runtime value type that can hold any telemetry data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TelemetryValue {
    /// An 8-bit character value (`irsdk_char`).
    Char(u8),
    /// An 8-bit signed integer produced by a derived or transformed value.
    Int8(i8),
    /// An 8-bit unsigned integer produced by a derived or transformed value.
    UInt8(u8),
    /// A 16-bit signed integer produced by a derived or transformed value.
    Int16(i16),
    /// A 16-bit unsigned integer produced by a derived or transformed value.
    UInt16(u16),
    /// A 32-bit signed integer (`irsdk_int`).
    Int32(i32),
    /// A 32-bit unsigned integer produced by a derived or transformed value.
    UInt32(u32),
    /// A 32-bit IEEE 754 floating-point value (`irsdk_float`).
    Float32(f32),
    /// A 64-bit IEEE 754 floating-point value (`irsdk_double`).
    Float64(f64),
    /// A boolean value (`irsdk_bool`).
    Bool(bool),
    /// A 32-bit bitfield (`irsdk_bitField`).
    BitField(BitField),
    /// An array of homogeneous telemetry values (multi-element variables).
    Array(Vec<TelemetryValue>),
}

impl TelemetryValue {
    /// Decodes requested VariableInfo from the provided data.
    pub fn decode(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        match info.count {
            0 => Ok(Self::Array(Vec::new())),
            1 => Self::decode_scalar(data, info),
            _ => Self::decode_array(data, info),
        }
    }

    fn decode_scalar(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        match info.data_type {
            VariableType::Character => u8::from_bytes(data, info).map(Self::Char),
            VariableType::Integer => i32::from_bytes(data, info).map(Self::Int32),
            VariableType::Float => f32::from_bytes(data, info).map(Self::Float32),
            VariableType::Double => f64::from_bytes(data, info).map(Self::Float64),
            VariableType::Boolean => bool::from_bytes(data, info).map(Self::Bool),
            VariableType::BitField => BitField::from_bytes(data, info).map(Self::BitField),
            VariableType::ElementTypeCount => Err(IRacingSDKError::type_conversion(
                "telemetry storage type",
                info.data_type,
            )),
        }
    }

    fn decode_array(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        let element_size = info.data_type.byte_size().ok_or_else(|| {
            IRacingSDKError::type_conversion("telemetry storage type", info.data_type)
        })?;
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
    fn telemetry_value(&self, info: &VariableInfo) -> crate::Result<TelemetryValue>;
}
