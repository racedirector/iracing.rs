//! Telemetry variable type definitions

#[cfg(feature = "codegen")]
use schemars::{JsonSchema, Schema, json_schema};
use serde::{Deserialize, Serialize};

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

/// Runtime value type that can hold any telemetry data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
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
    Array(Vec<Value>),
}

#[cfg(test)]
mod tests {
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
