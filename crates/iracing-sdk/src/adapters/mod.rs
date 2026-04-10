//! Type-safe frame adapters for converting raw telemetry to strongly-typed structures.
//!
//! This module provides a dual-phase adapter system:
//! - **Connection-time validation**: Validates field mappings and builds extraction plans
//! - **Runtime extraction**: Zero-overhead field extraction using pre-validated plans
//!
//! # Design Philosophy
//!
//! The adapter system follows a "fail-fast, run-fast" principle:
//! - All field mapping errors are caught at connection time with helpful suggestions
//! - Runtime extraction has zero HashMap lookups and minimal overhead (<1ms target)
//! - Type safety is enforced through integration with the existing VarData trait
//!
//! # Example Usage
//!
//! ```rust
//! use iracing_sdk::{AdapterValidation, FieldExtraction, FrameAdapter, FramePacket, IRacingSDKError, VarData, VariableSchema};
//!
//! // Manual adapter implementation
//! struct CarData {
//!     speed: f32,
//!     rpm: i32,
//!     gear: Option<i32>,
//! }
//!
//! impl FrameAdapter for CarData {
//!     fn validate_schema(schema: &VariableSchema) -> iracing_sdk::Result<AdapterValidation> {
//!         let mut extraction_plan = Vec::new();
//!
//!         // Validate required fields exist
//!         let speed_info = schema.get_variable("Speed")
//!             .ok_or_else(|| IRacingSDKError::Parse {
//!                 context: "Field validation".to_string(),
//!                 details: "Missing required field 'Speed'".to_string(),
//!             })?;
//!
//!         extraction_plan.push(FieldExtraction::Required {
//!             name: "Speed".to_string(),
//!             var_info: speed_info.clone(),
//!         });
//!
//!         Ok(AdapterValidation::new(extraction_plan))
//!     }
//!
//!     fn adapt(packet: &FramePacket, validation: &AdapterValidation) -> Self {
//!         // Required fields use the pre-validated plan for direct reads
//!         let speed = validation.fetch_or_default::<f32>(packet, "Speed");
//!         let rpm = validation.fetch_or_default::<i32>(packet, "RPM");
//!
//!         // Optional fields check the extraction plan for a mapped VariableInfo
//!         let gear = validation
//!             .index_of("Gear")
//!             .and_then(|idx| validation.extraction_plan.get(idx))
//!             .and_then(|field| field.var_info())
//!             .and_then(|info| i32::from_bytes(packet.data.as_ref(), info).ok());
//!
//!         Self { speed, rpm, gear }
//!     }
//! }
//! ```

mod frame_adapter;
mod schema_provider;
mod validation;

// Re-export all public types
pub use frame_adapter::FrameAdapter;
pub use schema_provider::SchemaProvider;
pub use validation::{AdapterValidation, DefaultValue, FieldExtraction};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VariableInfo, VariableSchema, VariableType};
    use std::collections::HashMap;

    fn create_test_schema() -> VariableSchema {
        let mut variables = HashMap::new();

        variables.insert(
            "Speed".to_string(),
            VariableInfo {
                name: "Speed".to_string(),
                data_type: VariableType::Float32,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "mph".to_string(),
                description: "Car speed".to_string(),
            },
        );

        variables.insert(
            "RPM".to_string(),
            VariableInfo {
                name: "RPM".to_string(),
                data_type: VariableType::Int32,
                offset: 4,
                count: 1,
                count_as_time: false,
                units: "rpm".to_string(),
                description: "Engine RPM".to_string(),
            },
        );

        VariableSchema::new(variables, 8).unwrap()
    }

    #[test]
    fn adapter_validation_creation() {
        let extraction_plan = vec![FieldExtraction::Required {
            name: "Speed".to_string(),
            var_info: VariableInfo {
                name: "Speed".to_string(),
                data_type: VariableType::Float32,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "mph".to_string(),
                description: "Car speed".to_string(),
            },
        }];

        let validation = AdapterValidation::new(extraction_plan);
        assert_eq!(validation.field_count(), 1);
        assert!(validation.has_required_fields());
        assert_eq!(validation.index_of("Speed"), Some(0));
        assert_eq!(validation.index_of("RPM"), None);
    }

    #[test]
    fn field_extraction_properties() {
        let required_field = FieldExtraction::Required {
            name: "Speed".to_string(),
            var_info: VariableInfo {
                name: "Speed".to_string(),
                data_type: VariableType::Float32,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "mph".to_string(),
                description: "Car speed".to_string(),
            },
        };

        assert_eq!(required_field.field_name(), Some("Speed"));
        assert!(required_field.is_required());
        assert!(required_field.var_info().is_some());

        let skipped_field = FieldExtraction::Skipped;
        assert_eq!(skipped_field.field_name(), None);
        assert!(!skipped_field.is_required());
        assert!(skipped_field.var_info().is_none());
    }

    #[test]
    fn schema_provider_basic_usage() {
        struct TestProvider {
            schema: VariableSchema,
        }

        impl SchemaProvider for TestProvider {
            fn get_schema(&self) -> &VariableSchema {
                &self.schema
            }
        }

        let provider = TestProvider {
            schema: create_test_schema(),
        };

        assert!(provider.has_field("Speed"));
        assert!(!provider.has_field("InvalidField"));
        assert!(provider.get_field_info("Speed").is_some());

        let field_names = provider.get_field_names();
        assert!(field_names.contains(&"Speed".to_string()));
        assert!(field_names.contains(&"RPM".to_string()));
    }
}
