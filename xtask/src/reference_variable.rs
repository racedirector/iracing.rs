//! Historical reference-artifact DTO; never used for runtime telemetry.
#![allow(dead_code)]
/// # Variable info
/// Information about a specific telemetry variable.
#[derive(schemars::JsonSchema)]
#[schemars(rename = "VariableInfo")]
pub(crate) struct ReferenceVariable {
    /// # Name
    /// Variable name as defined by iRacing
    pub name: String,
    /// # Data type
    /// Data type of the variable
    #[schemars(schema_with = "storage_type_schema")]
    pub data_type: iracing_irsdk::VariableType,
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

fn storage_type_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "enum": ["Character", "Boolean", "Integer", "BitField", "Float", "Double"]
    })
}
