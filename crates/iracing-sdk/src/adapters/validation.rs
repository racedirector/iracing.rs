//! Validation types and field extraction strategies for adapters

#[allow(unused_imports)] // Used by generated derive macro code
use crate::{IRacingSDKError, VariableInfo, VariableSchema};
#[allow(unused_imports)] // Used by generated derive macro code and tests
use std::collections::HashMap;

/// Pre-computed extraction plan built during connection-time validation.
///
/// Contains all information needed for efficient runtime extraction:
/// - Field offsets and type information
/// - Default values for missing optional fields
/// - Calculated field expressions (pre-parsed)
/// - Field extraction strategies per adapter field
#[derive(Debug, Clone)]
pub struct AdapterValidation {
    /// Ordered list of field extraction operations
    pub extraction_plan: Vec<FieldExtraction>,
    /// Fast lookup from telemetry field name to extraction index
    index_map: HashMap<String, usize>,
}

impl AdapterValidation {
    /// Create a new validation plan with the given extraction operations.
    pub fn new(extraction_plan: Vec<FieldExtraction>) -> Self {
        let index_map = extraction_plan
            .iter()
            .enumerate()
            .filter_map(|(index, extraction)| {
                extraction
                    .field_name()
                    .map(|name| (name.to_string(), index))
            })
            .collect();

        Self {
            extraction_plan,
            index_map,
        }
    }

    /// Get the number of fields that will be extracted.
    pub fn field_count(&self) -> usize {
        self.extraction_plan.len()
    }

    /// Check if the validation plan contains any required fields.
    pub fn has_required_fields(&self) -> bool {
        self.extraction_plan
            .iter()
            .any(|field| matches!(field, FieldExtraction::Required { .. }))
    }

    /// Lookup the extraction index for a telemetry field name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.index_map.get(name).copied()
    }

    /// Fetches a telemetry value by name using the adapter's extraction plan, falling back to the packet schema and ultimately the type default when unavailable.
    ///
    /// Attempts to decode the value using the precomputed extraction plan entry for `name` (if present) and its associated `VariableInfo`; if that fails, attempts to decode using `packet.schema.get_variable(name)`. If decoding succeeds returns the decoded value; otherwise returns `T::default()`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assume `adapter: AdapterValidation`, `packet: FramePacket`
    /// let speed: f32 = adapter.fetch_or_default(&packet, "speed");
    /// // `speed` is the decoded telemetry value or `f32::default()` if missing/invalid
    /// ```
    pub fn fetch_or_default<T>(&self, packet: &crate::FramePacket, name: &str) -> T
    where
        T: crate::VarData + ::core::default::Default,
    {
        let data = packet.data.as_ref();

        if let Some(index) = self.index_of(name) {
            if let Some(entry) = self.extraction_plan.get(index) {
                if let Some(var_info) = entry.var_info() {
                    if let Ok(value) = <T as crate::VarData>::from_bytes(data, var_info) {
                        return value;
                    }
                }
            }
        }

        if let Some(var_info) = packet.schema.get_variable(name) {
            if let Ok(value) = <T as crate::VarData>::from_bytes(data, var_info) {
                return value;
            }
        }

        T::default()
    }
}

/// Determine whether a schema variable is incompatible with a target `VarData` type.
///
/// This probes type compatibility by calling `<T as VarData>::from_bytes(&[], var_info)`,
/// which performs type checks without requiring a real frame buffer.
///
/// # Returns
///
/// - `Ok(None)` if the variable can be mapped to `T` (including when the probe hits a memory/bounds condition).
/// - `Ok(Some(details))` if the probe fails with a type-conversion error; `details` contains the diagnostic message.
/// - `Err(err)` for any other error encountered while probing.
///
/// # Examples
///
/// ```no_run
/// # use iracing_sdk::adapters::validation::telemetry_type_mismatch_details;
/// # use iracing_sdk::VariableInfo;
/// let var_info = VariableInfo::default();
/// let _ = telemetry_type_mismatch_details::<f32>(&var_info);
/// ```
#[doc(hidden)]
pub fn telemetry_type_mismatch_details<T>(var_info: &VariableInfo) -> crate::Result<Option<String>>
where
    T: crate::VarData,
{
    match <T as crate::VarData>::from_bytes(&[], var_info) {
        Ok(_) | Err(IRacingSDKError::Memory { .. }) => Ok(None),
        Err(IRacingSDKError::TypeConversion { details }) => Ok(Some(details)),
        Err(err) => Err(err),
    }
}

/// Extraction strategy for a single adapter field.
///
/// Strategy is determined at connection time based on field annotations,
/// field type (`Option<T>` vs `T`), and schema availability.
#[derive(Debug, Clone)]
pub enum FieldExtraction {
    /// Required field that must exist in schema - connection fails if missing.
    Required {
        /// Field name in telemetry schema
        name: String,
        /// Variable metadata from schema
        var_info: VariableInfo,
    },

    /// Optional field that may or may not exist in schema.
    Optional {
        /// Field name in telemetry schema
        name: String,
        /// Variable metadata if field exists, None if missing
        var_info: Option<VariableInfo>,
    },

    /// Field with custom default value when missing from schema.
    WithDefault {
        /// Field name in telemetry schema
        name: String,
        /// Variable metadata if field exists, None if missing
        var_info: Option<VariableInfo>,
        /// Strategy used to produce the fallback value
        default_value: DefaultValue,
    },

    /// Calculated field derived from other fields or expressions.
    Calculated {
        /// Expression to evaluate (e.g., "speed_mph * 1.60934")
        expression: String,
    },

    /// Field to skip during extraction (application-managed).
    Skipped,
}

impl FieldExtraction {
    /// Get the telemetry field name if this extraction involves a telemetry field.
    pub fn field_name(&self) -> Option<&str> {
        match self {
            FieldExtraction::Required { name, .. }
            | FieldExtraction::Optional { name, .. }
            | FieldExtraction::WithDefault { name, .. } => Some(name),
            FieldExtraction::Calculated { .. } | FieldExtraction::Skipped => None,
        }
    }

    /// Check if this field extraction requires the field to exist in the schema.
    pub fn is_required(&self) -> bool {
        matches!(self, FieldExtraction::Required { .. })
    }

    /// Get the variable info for this field if available.
    pub fn var_info(&self) -> Option<&VariableInfo> {
        match self {
            FieldExtraction::Required { var_info, .. } => Some(var_info),
            FieldExtraction::Optional { var_info, .. }
            | FieldExtraction::WithDefault { var_info, .. } => var_info.as_ref(),
            FieldExtraction::Calculated { .. } | FieldExtraction::Skipped => None,
        }
    }
}

/// Describes how a default value should be produced when telemetry data is unavailable.
#[derive(Debug, Clone)]
pub enum DefaultValue {
    /// Use the `Default` implementation of the target field type.
    TypeDefault,
    /// Evaluate a user-provided expression supplied via `#[missing = "..."]`.
    ExplicitExpression(String),
}

impl DefaultValue {
    /// Human readable description of the defaulting strategy.
    pub fn describe(&self) -> &'static str {
        match self {
            DefaultValue::TypeDefault => "type default",
            DefaultValue::ExplicitExpression(_) => "explicit expression",
        }
    }
}
