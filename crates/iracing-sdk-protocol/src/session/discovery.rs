//! Schema discovery utilities for unknown fields
//!
//! This module provides types and helpers for discovering unknown fields during
//! session info parsing. Only available when the `schema-discovery` feature is enabled.

use serde_yaml_ng::Value;

/// Report of an unknown field discovered during schema parsing
#[derive(Debug, Clone, PartialEq)]
pub struct UnknownField {
    /// JSON path to the field (e.g., "WeekendInfo.TelemetryOptions.NewField")
    pub path: String,
    /// Data type of the field
    pub data_type: UnknownFieldType,
    /// Example value (for leaf nodes) or structure description
    pub example: String,
}

/// Types of unknown fields that can be discovered
#[derive(Debug, Clone, PartialEq)]
pub enum UnknownFieldType {
    /// String value
    String,
    /// Numeric value (int or float)
    Number,
    /// Boolean value
    Boolean,
    /// Null value
    Null,
    /// Object/map
    Object,
    /// Array/list
    Array,
}

/// Convert serde_yaml_ng::Value to UnknownFieldType
pub fn value_to_type(value: &Value) -> UnknownFieldType {
    match value {
        Value::String(_) => UnknownFieldType::String,
        Value::Number(_) => UnknownFieldType::Number,
        Value::Bool(_) => UnknownFieldType::Boolean,
        Value::Null => UnknownFieldType::Null,
        Value::Mapping(_) => UnknownFieldType::Object,
        Value::Sequence(_) => UnknownFieldType::Array,
        Value::Tagged(tagged) => value_to_type(&tagged.value),
    }
}

/// Convert serde_yaml_ng::Value to example string (truncate strings to 100 chars)
pub fn value_to_example(value: &Value) -> String {
    match value {
        Value::String(s) => {
            if s.len() > 100 {
                format!("{}...", &s[..100])
            } else {
                s.clone()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Mapping(_) => "{...}".to_string(),
        Value::Sequence(seq) => format!("[{} items]", seq.len()),
        Value::Tagged(tagged) => {
            format!("!{} {}", tagged.tag, value_to_example(&tagged.value))
        }
    }
}

/// Recursively collect all leaf fields from a YAML value
///
/// This function traverses objects and arrays to find all leaf nodes (primitives)
/// and reports them with their full paths. For example, if you have:
/// ```yaml
/// QualifyResultsInfo:
///   Results:
///     - Position: 1
///       ClassPosition: 1
/// ```
///
/// It will report:
/// - `QualifyResultsInfo.Results[0].Position: Number = 1`
/// - `QualifyResultsInfo.Results[0].ClassPosition: Number = 1`
///
/// Instead of just:
/// - `QualifyResultsInfo: Object = {...}`
pub fn collect_leaf_fields(base_path: &str, value: &Value) -> Vec<UnknownField> {
    let mut fields = Vec::new();

    match value {
        // Leaf nodes - primitives
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
            fields.push(UnknownField {
                path: base_path.to_string(),
                data_type: value_to_type(value),
                example: value_to_example(value),
            });
        }

        // Objects - recurse into each key-value pair
        Value::Mapping(map) => {
            for (key, val) in map {
                if let Some(key_str) = key.as_str() {
                    let child_path = if base_path.is_empty() {
                        key_str.to_string()
                    } else {
                        format!("{}.{}", base_path, key_str)
                    };
                    fields.extend(collect_leaf_fields(&child_path, val));
                }
            }
        }

        // Arrays - recurse into each element with [index]
        Value::Sequence(seq) => {
            for (i, val) in seq.iter().enumerate() {
                let child_path = format!("{}[{}]", base_path, i);
                fields.extend(collect_leaf_fields(&child_path, val));
            }
        }

        // Tagged values - unwrap and recurse
        Value::Tagged(tagged) => {
            fields.extend(collect_leaf_fields(base_path, &tagged.value));
        }
    }

    fields
}
