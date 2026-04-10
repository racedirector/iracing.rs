//! Schema provider trait for telemetry sources

use crate::{VariableInfo, VariableSchema};

/// Provider abstraction for schema discovery across different telemetry sources.
///
/// This trait enables adapters to work with any telemetry source (live iRacing,
/// IBT files, test data) by abstracting schema access.
pub trait SchemaProvider {
    /// Get the variable schema for this telemetry source.
    fn get_schema(&self) -> &VariableSchema;

    /// Check if a field exists in the schema.
    fn has_field(&self, name: &str) -> bool {
        self.get_schema().get_variable(name).is_some()
    }

    /// Get variable information for a field name.
    fn get_field_info(&self, name: &str) -> Option<&VariableInfo> {
        self.get_schema().get_variable(name)
    }

    /// Get all available field names in this schema.
    fn get_field_names(&self) -> Vec<String> {
        self.get_schema().variables.keys().cloned().collect()
    }
}
