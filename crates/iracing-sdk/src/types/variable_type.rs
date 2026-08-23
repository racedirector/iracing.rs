//! Telemetry variable type definitions

use std::fmt;

#[cfg(feature = "codegen")]
use schemars::{JsonSchema, Schema, json_schema};
use serde::{Deserialize, Serialize};

use crate::{BitField, IRacingSDKError, Result, VarData, VariableInfo};

fn variable_header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Variable header validation", details)
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrsdkVarType {
    Char = 0,
    Bool = 1,
    Int = 2,
    BitField = 3,
    Float = 4,
    Double = 5,
}

impl IrsdkVarType {
    pub const fn size(&self) -> usize {
        match self {
            IrsdkVarType::Char | IrsdkVarType::Bool | IrsdkVarType::Int => 1,
            IrsdkVarType::BitField | IrsdkVarType::Float => 4,
            IrsdkVarType::Double => 8,
        }
    }
}

impl TryFrom<i32> for IrsdkVarType {
    type Error = IRacingSDKError;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::Char),
            1 => Ok(Self::Bool),
            2 => Ok(Self::Int),
            3 => Ok(Self::BitField),
            4 => Ok(Self::Float),
            5 => Ok(Self::Double),
            value => Err(variable_header_validation_error(format!(
                "Invalid variable type: {value}"
            ))),
        }
    }
}

/// Supported telemetry data types.
/// Maps to iRacing SDK's irsdk_VarType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
pub enum VariableType {
    /// 8-bit character (maps to irsdk_char)
    Char,
    /// 8-bit signed integer
    Int8,
    /// 8-bit unsigned integer
    UInt8,
    /// 16-bit signed integer
    Int16,
    /// 16-bit unsigned integer
    UInt16,
    /// 32-bit signed integer (maps to irsdk_int)
    Int32,
    /// 32-bit unsigned integer
    UInt32,
    /// 32-bit floating point (maps to irsdk_float)
    Float32,
    /// 64-bit floating point (maps to irsdk_double)
    Float64,
    /// Boolean value (maps to irsdk_bool)
    Bool,
    /// 32-bit bitfield (maps to irsdk_bitField)
    BitField,
}

impl fmt::Display for VariableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl VariableType {
    /// Returns the size in bytes of this data type.
    /// Matches the irsdk_VarTypeBytes array from the iRacing SDK.
    pub const fn size(&self) -> usize {
        match self {
            VariableType::Char | VariableType::Bool => 1,
            VariableType::Int8 | VariableType::UInt8 => 1,
            VariableType::Int16 | VariableType::UInt16 => 2,
            VariableType::Int32
            | VariableType::UInt32
            | VariableType::Float32
            | VariableType::BitField => 4,
            VariableType::Float64 => 8,
        }
    }
}

impl From<i32> for VariableType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Char,
            1 => Self::Bool,
            2 => Self::Int32,
            3 => Self::BitField,
            4 => Self::Float32,
            5 => Self::Float64,
            _ => {
                tracing::warn!(value, "Unknown iRacing variable type, defaulting to Int32");
                VariableType::Int32
            }
        }
    }
}

#[cfg(feature = "codegen")]
impl From<VariableType> for Schema {
    fn from(value: VariableType) -> Self {
        let type_value = match value {
            VariableType::Char => "string",
            VariableType::Bool => "boolean",
            VariableType::Float32 | VariableType::Float64 => "number",
            VariableType::Int8
            | VariableType::UInt8
            | VariableType::Int16
            | VariableType::UInt16
            | VariableType::Int32
            | VariableType::UInt32
            | VariableType::BitField => "integer",
        };

        json_schema!({
            "type": type_value
        })
    }
}

impl From<IrsdkVarType> for VariableType {
    fn from(value: IrsdkVarType) -> Self {
        match value {
            IrsdkVarType::Char => Self::Char,
            IrsdkVarType::Bool => Self::Bool,
            IrsdkVarType::Int => Self::Int32,
            IrsdkVarType::BitField => Self::BitField,
            IrsdkVarType::Float => Self::Float32,
            IrsdkVarType::Double => Self::Float64,
        }
    }
}

/// Runtime value type that can hold any telemetry data.
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
            VariableType::Char => u8::from_bytes(data, info).map(Self::Char),
            VariableType::Int8 => i8::from_bytes(data, info).map(Self::Int8),
            VariableType::UInt8 => u8::from_bytes(data, info).map(Self::UInt8),
            VariableType::Int16 => i16::from_bytes(data, info).map(Self::Int16),
            VariableType::UInt16 => u16::from_bytes(data, info).map(Self::UInt16),
            VariableType::Int32 => i32::from_bytes(data, info).map(Self::Int32),
            VariableType::UInt32 => u32::from_bytes(data, info).map(Self::UInt32),
            VariableType::Float32 => f32::from_bytes(data, info).map(Self::Float32),
            VariableType::Float64 => f64::from_bytes(data, info).map(Self::Float64),
            VariableType::Bool => bool::from_bytes(data, info).map(Self::Bool),
            VariableType::BitField => BitField::from_bytes(data, info).map(Self::BitField),
        }
    }

    fn decode_array(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        let element_size = info.data_type.size();
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

#[cfg(test)]
mod tests {
    #[cfg(feature = "codegen")]
    use super::*;

    #[cfg(feature = "codegen")]
    #[test]
    fn from_variable_type_to_schema_maps_to_expected_json_type() {
        let cases = [
            (VariableType::Char, "string"),
            (VariableType::Bool, "boolean"),
            (VariableType::Float32, "number"),
            (VariableType::Float64, "number"),
            (VariableType::Int8, "integer"),
            (VariableType::UInt8, "integer"),
            (VariableType::Int16, "integer"),
            (VariableType::UInt16, "integer"),
            (VariableType::Int32, "integer"),
            (VariableType::UInt32, "integer"),
            (VariableType::BitField, "integer"),
        ];

        for (var_type, expected_type) in cases {
            let schema = Schema::from(var_type);
            assert_eq!(
                schema,
                json_schema!({ "type": expected_type }),
                "{var_type:?} should map to JSON Schema type '{expected_type}'"
            );
        }
    }
}
