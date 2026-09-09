//! Telemetry variable schema types

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    IRacingSDKError, Result,
    irsdk::{VariableHeader, VariableType as IRSDKVariableType},
    parse_utils,
};

use super::variable_headers_buffer::VariableHeadersBuffer;

fn schema_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Schema validation", details)
}

/// # Variable info
/// Information about a specific telemetry variable.
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    /// # Name
    /// Variable name as defined by iRacing
    pub name: String,
    /// # Data type
    /// Data type of the variable
    #[cfg_attr(feature = "codegen", schemars(schema_with = "storage_type_schema"))]
    pub data_type: IRSDKVariableType,
    /// # Byte offset
    /// Byte offset within the telemetry frame
    pub offset: usize,
    /// # Count
    /// Number of elements (1 for scalar, >1 for arrays)
    pub count: usize,
    /// # Count as time
    /// Whether the simulator treats the sample count as elapsed time
    pub count_as_time: bool,
    /// # Units
    /// Units of measurement (e.g., "m/s", "C", "N*m")
    pub units: String,
    /// # Description
    /// Human-readable description
    pub description: String,
}

impl VariableInfo {
    pub(crate) fn storage_byte_size(&self) -> Result<usize> {
        self.data_type.byte_size().ok_or_else(|| {
            schema_validation_error("ElementTypeCount cannot describe a telemetry variable")
        })
    }
}

impl TryFrom<&VariableHeader> for VariableInfo {
    type Error = IRacingSDKError;

    fn try_from(value: &VariableHeader) -> Result<Self> {
        Ok(VariableInfo {
            name: parse_utils::c_string_to_string(&value.name),
            description: parse_utils::c_string_to_string(&value.description),
            units: parse_utils::c_string_to_string(&value.unit),
            offset: usize::try_from(value.offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "VariableInfo::try_from",
                    format!("Could not convert {} to usize", value.offset),
                )
            })?,
            count: usize::try_from(value.count).map_err(|_| {
                IRacingSDKError::parse_error(
                    "VariableInfo::try_from",
                    format!("Could not convert {} to usize", value.count,),
                )
            })?,
            count_as_time: value.count_as_time != 0,
            data_type: value.variable_type()?,
        })
    }
}

impl TryFrom<VariableHeader> for VariableInfo {
    type Error = IRacingSDKError;

    fn try_from(value: VariableHeader) -> Result<Self> {
        Self::try_from(&value)
    }
}

/// # Variable schema
/// Schema describing the structure and metadata of telemetry variables.
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSchema {
    /// # Variables map
    /// Map of variable names to their metadata (provides O(1) lookup)
    pub variables: HashMap<String, VariableInfo>,
    /// # Frame size
    /// Total size of a telemetry frame in bytes
    pub frame_size: usize,
}

impl VariableSchema {
    /// Returns an empty schema.
    pub fn empty() -> Self {
        Self {
            variables: HashMap::new(),
            frame_size: 0,
        }
    }

    /// Create a new VariableSchema with validation.
    pub fn new(variables: HashMap<String, VariableInfo>, frame_size: usize) -> crate::Result<Self> {
        let schema = Self {
            variables,
            frame_size,
        };
        schema.validate()?;
        Ok(schema)
    }

    /// Constructs a schema from an exact snapshot of SDK variable headers.
    pub fn from_headers(headers: &VariableHeadersBuffer, frame_size: usize) -> crate::Result<Self> {
        let mut variables = HashMap::with_capacity(headers.iter_headers().len());

        for header in headers.iter_headers() {
            let variable = VariableInfo::try_from(header)?;

            if variable.name.is_empty() {
                return Err(schema_validation_error("Variable header has empty name"));
            }

            if variables.contains_key(&variable.name) {
                return Err(schema_validation_error(format!(
                    "Duplicate variable name '{}' in header region",
                    variable.name
                )));
            }

            variables.insert(variable.name.clone(), variable);
        }

        Self::new(variables, frame_size)
    }

    /// Validate the schema for consistency.
    pub fn validate(&self) -> crate::Result<()> {
        for (name, var_info) in &self.variables {
            // Validate variable count
            if var_info.count == 0 {
                return Err(schema_validation_error(format!(
                    "Variable '{}' has count of 0",
                    name
                )));
            }

            // Validate variable name matches info name
            if var_info.name != *name {
                return Err(schema_validation_error(format!(
                    "Variable map key '{}' doesn't match info name '{}'",
                    name, var_info.name
                )));
            }

            // Validate that variable fits within frame
            let end_offset = var_info
                .storage_byte_size()?
                .checked_mul(var_info.count)
                .and_then(|size| var_info.offset.checked_add(size))
                .ok_or_else(|| schema_validation_error("Variable extent overflows usize"))?;
            if end_offset > self.frame_size {
                return Err(IRacingSDKError::memory_access_error(var_info.offset));
            }
        }

        Ok(())
    }

    /// Get variable info by name (O(1) lookup).
    pub fn get_variable(&self, name: &str) -> Option<&VariableInfo> {
        self.variables.get(name)
    }

    /// Check if a variable exists.
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Get the number of variables.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Get the names of all available variables.
    pub fn variable_names(&self) -> Vec<String> {
        self.variables.keys().cloned().collect()
    }

    /// Get all available variables in the schema.
    pub fn variables(&self) -> Vec<VariableInfo> {
        self.variables.values().cloned().collect()
    }
}

/// Provider abstraction for schema discovery across telemetry sources.
///
/// This trait enables consumers to work with any telemetry source (live iRacing,
/// IBT files, or test data) by abstracting schema access.
pub trait SchemaProvider {
    /// Get the variable schema for this telemetry source.
    fn schema(&self) -> &VariableSchema;

    /// Get variable information for a field name.
    fn variable(&self, name: &str) -> Option<&VariableInfo> {
        self.schema().get_variable(name)
    }

    /// Check if a field exists in the schema.
    fn has_variable(&self, name: &str) -> bool {
        self.schema().has_variable(name)
    }

    /// Get all available field names in this schema.
    fn variable_names(&self) -> Vec<String> {
        self.schema().variable_names()
    }

    /// Get all available variable values.
    fn variables(&self) -> Vec<VariableInfo> {
        self.schema().variables()
    }

    /// The number of variables in the schema.
    fn variable_count(&self) -> usize {
        self.schema().variable_count()
    }
}

#[cfg(feature = "codegen")]
fn storage_type_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "enum": ["Character", "Boolean", "Integer", "BitField", "Float", "Double"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{irsdk::VariableType as IRSDKVariableType, irsdk::WireType};

    struct TestProvider {
        schema: VariableSchema,
    }

    impl SchemaProvider for TestProvider {
        fn schema(&self) -> &VariableSchema {
            &self.schema
        }
    }

    #[test]
    fn rejects_sentinel_and_overflowing_metadata() {
        use crate::{TelemetryValue, VarData};
        let mut header = VariableHeader::new(
            IRSDKVariableType::Float,
            0,
            1,
            false,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap();
        let mut info = VariableInfo::try_from(&header).unwrap();
        header.variable_type = 6;
        assert!(VariableInfo::try_from(&header).is_err());
        info.data_type = IRSDKVariableType::ElementTypeCount;
        for count in [0, 1, 2] {
            info.count = count;
            assert!(TelemetryValue::decode(&[], &info).is_err());
            assert!(Vec::<f32>::from_bytes(&[], &info).is_err());
            assert!(
                VariableSchema::new(HashMap::from([("Speed".into(), info.clone())]), 4).is_err()
            );
        }
        info.data_type = IRSDKVariableType::Float;
        info.count = usize::MAX;
        assert!(
            VariableSchema::new(HashMap::from([("Speed".into(), info.clone())]), usize::MAX)
                .is_err()
        );
        info.count = 1;
        info.offset = usize::MAX;
        assert!(VariableSchema::new(HashMap::from([("Speed".into(), info)]), usize::MAX).is_err());
    }

    #[cfg(feature = "codegen")]
    #[test]
    fn metadata_schema_only_advertises_storage_types() {
        let schema = schemars::schema_for!(VariableInfo);
        let value = serde_json::to_value(schema).unwrap();
        assert_eq!(
            value["properties"]["data_type"]["enum"],
            serde_json::json!([
                "Character",
                "Boolean",
                "Integer",
                "BitField",
                "Float",
                "Double"
            ])
        );
    }

    #[test]
    fn schema_provider_basic_usage() {
        let speed = VariableInfo {
            name: "Speed".to_string(),
            data_type: IRSDKVariableType::Float,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: "mph".to_string(),
            description: "Car speed".to_string(),
        };
        let provider = TestProvider {
            schema: VariableSchema::new(HashMap::from([("Speed".to_string(), speed)]), 4).unwrap(),
        };

        assert!(provider.has_variable("Speed"));
        assert!(!provider.has_variable("InvalidField"));
        assert!(provider.variable("Speed").is_some());
        assert_eq!(provider.variable_names(), vec!["Speed".to_string()]);
    }

    #[test]
    fn constructs_schema_from_variable_headers_buffer() {
        let header = VariableHeader::new(
            IRSDKVariableType::Float,
            4,
            1,
            false,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap();
        let mut bytes = Vec::new();
        header.write_to(&mut bytes).unwrap();
        let headers = VariableHeadersBuffer::from_checked_region(&bytes);

        let schema = VariableSchema::from_headers(&headers, 8).unwrap();

        let speed = schema.get_variable("Speed").unwrap();
        assert_eq!(speed.offset, 4);
        assert_eq!(speed.count, 1);
        assert_eq!(schema.frame_size, 8);
    }
}
