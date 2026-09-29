//! Telemetry variable schema types

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

use crate::{IRacingSDKError, Result, VariableInfo, irsdk::VariableHeader};

use super::variable_headers_buffer::VariableHeadersBuffer;

fn schema_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Schema validation", details)
}

/// # Variable schema
/// Schema describing the structure and metadata of telemetry variables.
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[derive(Debug, Clone, Serialize)]
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
    pub fn new(variables: HashMap<String, VariableInfo>, frame_size: usize) -> Result<Self> {
        for (name, info) in &variables {
            if name.is_empty() || info.name.is_empty() {
                return Err(schema_validation_error("Variable name is empty"));
            }
            if *name != info.name {
                return Err(schema_validation_error(format!(
                    "Variable map key '{name}' doesn't match info name '{}'",
                    info.name
                )));
            }
            if info.region().as_region().end() > frame_size {
                return Err(schema_validation_error(format!(
                    "Variable '{name}' extends past frame size {frame_size}"
                )));
            }
        }
        Ok(Self {
            variables,
            frame_size,
        })
    }

    /// Constructs a schema from an exact snapshot of SDK variable headers.
    pub fn from_snapshot(snapshot: VariableHeadersBuffer, frame_size: usize) -> Result<Self> {
        Self::from_headers(snapshot.as_slice(), frame_size)
    }

    /// Constructs and validates a schema from decoded SDK variable headers.
    pub fn from_headers(headers: &[VariableHeader], frame_size: usize) -> Result<Self> {
        let mut variables = HashMap::with_capacity(headers.len());

        for header in headers.iter() {
            let variable = VariableInfo::try_from_header(header, frame_size)?;

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

impl<'de> Deserialize<'de> for VariableSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            variables: HashMap<String, VariableInfo>,
            frame_size: usize,
        }

        let fields = Fields::deserialize(deserializer)?;
        Self::new(fields.variables, fields.frame_size).map_err(serde::de::Error::custom)
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
    use zerocopy::IntoBytes;

    use super::*;
    use crate::irsdk::VariableType as IRSDKVariableType;

    struct TestProvider {
        schema: VariableSchema,
    }

    impl SchemaProvider for TestProvider {
        fn schema(&self) -> &VariableSchema {
            &self.schema
        }
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
        assert!(value["properties"].get("offset").is_some());
        assert!(value["properties"].get("count").is_some());
        assert!(value["properties"].get("region").is_none());
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

        let bytes = header.as_bytes();
        let headers = VariableHeadersBuffer::try_from_region_bytes(bytes, 1).unwrap();

        let schema = VariableSchema::from_snapshot(headers, 8).unwrap();

        let speed = schema.get_variable("Speed").unwrap();
        assert_eq!(speed.offset(), 4);
        assert_eq!(speed.count(), 1);
        assert_eq!(schema.frame_size, 8);
    }

    #[test]
    fn schema_preserves_semantic_checks() {
        let header =
            VariableHeader::new(IRSDKVariableType::Float, 0, 1, false, "Speed", "", "").unwrap();
        assert!(VariableSchema::from_headers(&[header, header], 4).is_err());
        let empty = VariableHeader::new(IRSDKVariableType::Float, 0, 1, false, "", "", "").unwrap();
        assert!(VariableSchema::from_headers(&[empty], 4).is_err());

        let info = VariableInfo::try_from_header(&header, 4).unwrap();
        assert!(VariableSchema::new(HashMap::from([("Other".into(), info.clone())]), 4).is_err());
        assert!(VariableSchema::new(HashMap::from([("Speed".into(), info)]), 3).is_err());
    }

    #[test]
    fn invalid_storage_type_is_rejected_at_wire_boundary() {
        let header =
            VariableHeader::new(IRSDKVariableType::Float, 0, 1, false, "Speed", "", "").unwrap();
        let mut bytes = header.as_bytes().to_vec();
        bytes[..4].copy_from_slice(&6_i32.to_le_bytes());
        assert!(VariableHeadersBuffer::try_from_region_bytes(&bytes, 1).is_err());
    }

    #[test]
    fn schema_deserialization_checks_frame_size() {
        let header =
            VariableHeader::new(IRSDKVariableType::Float, 4, 2, false, "Speed", "", "").unwrap();
        let schema = VariableSchema::from_headers(&[header], 12).unwrap();
        let value = serde_json::to_value(&schema).unwrap();
        let decoded: VariableSchema = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            decoded.get_variable("Speed").unwrap().region().as_range(),
            4..12
        );

        let mut invalid = value;
        invalid["frame_size"] = serde_json::json!(11);
        assert!(serde_json::from_value::<VariableSchema>(invalid).is_err());

        let provider = TestProvider { schema };
        assert!(provider.has_variable("Speed"));
    }
}
