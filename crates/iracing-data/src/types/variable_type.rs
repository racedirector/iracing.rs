//! Telemetry variable type definitions

use serde::{Deserialize, Serialize};

/// Supported telemetry data types.
/// Maps to iRacing SDK's irsdk_VarType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Runtime value type that can hold any telemetry data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Char(u8),
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    BitField(super::BitField),
    Array(Vec<Value>),
}
