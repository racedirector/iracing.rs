//! Telemetry variable schema types

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[cfg(windows)]
use crate::WindowsConnection;
use crate::{
    IRacingSDKError, Result, parse_utils, reader::ibt::IbtReader, types::VariableHeadersBuffer,
};

use super::{VariableType, irsdk::VariableHeader};

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
    pub data_type: VariableType,
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

impl TryFrom<VariableHeader> for VariableInfo {
    type Error = IRacingSDKError;

    /// Convert the wire-format VariableHeader into library type `VariableInfo`.
    fn try_from(value: VariableHeader) -> Result<Self> {
        value.validate()?;

        Ok(VariableInfo {
            name: parse_utils::c_string_to_string(&value.name),
            description: parse_utils::c_string_to_string(&value.description),
            units: parse_utils::c_string_to_string(&value.unit),
            data_type: value.variable_type()?,
            offset: usize::try_from(value.offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "TryFrom<VariableHeader> for VariableInfo",
                    format!("Could not convert {} to usize", value.offset),
                )
            })?,
            count: usize::try_from(value.count).map_err(|_| {
                IRacingSDKError::parse_error(
                    "TryFrom<VariableHeader> for VariableInfo",
                    format!("Could not convert {} to usize", value.count,),
                )
            })?,
            count_as_time: value.count_as_time != 0,
        })
    }
}

impl TryFrom<VariableHeadersBuffer> for Vec<VariableInfo> {
    type Error = IRacingSDKError;

    fn try_from(value: VariableHeadersBuffer) -> Result<Self> {
        let variables = value
            .iter_headers()
            .map(VariableInfo::try_from)
            .collect::<Result<Vec<VariableInfo>>>()?;

        Ok(variables)
    }
}

#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// Owned variable metadata keyed by its wire-format name.
pub struct VariablesHashMap(HashMap<String, VariableInfo>);

impl Deref for VariablesHashMap {
    type Target = HashMap<String, VariableInfo>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VariablesHashMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<HashMap<String, VariableInfo>> for VariablesHashMap {
    fn from(value: HashMap<String, VariableInfo>) -> Self {
        Self(value)
    }
}

impl TryFrom<VariableHeadersBuffer> for VariablesHashMap {
    type Error = IRacingSDKError;

    fn try_from(value: VariableHeadersBuffer) -> Result<Self> {
        let variables = value
            .iter_headers()
            .map(VariableInfo::try_from)
            .map(|result| {
                let info = result?;
                Ok((info.name.clone(), info))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(Self(variables))
    }
}

/// # Variable schema
/// Schema describing the structure and metadata of telemetry variables.
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct VariableSchema {
    /// # Variables map
    /// Map of variable names to their metadata (provides O(1) lookup)
    pub variables: VariablesHashMap,
    /// # Frame size
    /// Total size of a telemetry frame in bytes
    pub frame_size: usize,
}

impl VariableSchema {
    /// Builds a validated schema from an owned variable-header snapshot.
    pub fn from_variable_headers(
        variable_headers: VariableHeadersBuffer,
        frame_size: usize,
    ) -> Result<Self> {
        Self::new(VariablesHashMap::try_from(variable_headers)?, frame_size)
    }

    /// Create a new VariableSchema with validation.
    pub fn new(variables: impl Into<VariablesHashMap>, frame_size: usize) -> Result<Self> {
        let schema = Self {
            variables: variables.into(),
            frame_size,
        };

        schema.validate()?;
        Ok(schema)
    }

    /// Validate the schema for consistency.
    pub fn validate(&self) -> Result<()> {
        for (name, var_info) in &self.variables.0 {
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
            let element_size = var_info.data_type.byte_size().ok_or_else(|| {
                schema_validation_error(format!(
                    "Variable '{}' uses the non-storage ElementTypeCount sentinel",
                    name
                ))
            })?;
            let end_offset = var_info.offset + (element_size * var_info.count);
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

#[cfg(windows)]
impl VariableSchema {
    /// Creates a VariableSchema from components of a WindowsConnection.
    pub fn from_connection(connection: &WindowsConnection) -> Result<Self> {
        let header = connection.header_snapshot();
        let variable_headers =
            connection
                .variable_headers_buffer()
                .ok_or(IRacingSDKError::parse_error(
                    "VariableSchema",
                    "Could not find variable headers from connection",
                ))?;

        Ok(Self {
            variables: variable_headers.try_into()?,
            frame_size: usize::try_from(header.buffer_length).map_err(|_| {
                IRacingSDKError::parse_error(
                    "VariableSchema",
                    format!("Could not convert {} to usize", header.buffer_length),
                )
            })?,
        })
    }
}

impl VariableSchema {
    /// Creates a VariableSchema from components of an `IbtReader`.
    pub fn from_reader(reader: &IbtReader) -> Result<Self> {
        let variable_headers =
            reader
                .variable_headers_buffer()?
                .ok_or(IRacingSDKError::parse_error(
                    "VariableSchema",
                    "Could not find variable headers from IbtReader",
                ))?;

        VariableSchema::from_variable_headers(variable_headers, reader.recording().frame_length())
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider {
        schema: VariableSchema,
    }

    impl SchemaProvider for TestProvider {
        fn schema(&self) -> &VariableSchema {
            &self.schema
        }
    }

    #[test]
    fn schema_provider_basic_usage() {
        let speed = VariableInfo {
            name: "Speed".to_string(),
            data_type: VariableType::Float,
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
}
