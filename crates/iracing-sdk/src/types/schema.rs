//! Telemetry variable schema types

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::VariableType;

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
    /// Create a new VariableSchema with validation.
    pub fn new(variables: HashMap<String, VariableInfo>, frame_size: usize) -> crate::Result<Self> {
        let schema = Self {
            variables,
            frame_size,
        };
        schema.validate()?;
        Ok(schema)
    }

    /// Validate the schema for consistency.
    pub fn validate(&self) -> crate::Result<()> {
        for (name, var_info) in &self.variables {
            // Validate variable count
            if var_info.count == 0 {
                return Err(crate::IRacingSDKError::Parse {
                    context: "Schema validation".to_string(),
                    details: format!("Variable '{}' has count of 0", name),
                });
            }

            // Validate variable name matches info name
            if var_info.name != *name {
                return Err(crate::IRacingSDKError::Parse {
                    context: "Schema validation".to_string(),
                    details: format!(
                        "Variable map key '{}' doesn't match info name '{}'",
                        name, var_info.name
                    ),
                });
            }

            // Validate that variable fits within frame
            let end_offset = var_info.offset + (var_info.data_type.size() * var_info.count);
            if end_offset > self.frame_size {
                return Err(crate::IRacingSDKError::Memory {
                    offset: var_info.offset,
                    source: None,
                });
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
    /// # Unites
    /// Units of measurement (e.g., "m/s", "C", "N*m")
    pub units: String,
    /// # Description
    /// Human-readable description
    pub description: String,
}
