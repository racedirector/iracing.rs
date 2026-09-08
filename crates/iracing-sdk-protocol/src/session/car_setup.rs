//! Car setup structures
//!
//! This module contains car setup information.

use serde::{Deserialize, Serialize};

/// Car setup information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(schemars::JsonSchema))]
#[serde(rename_all = "PascalCase")]
pub struct CarSetup {
    /// Number of times the setup has been updated this session
    pub update_count: i32,

    /// Car setup fields
    #[serde(flatten)]
    #[cfg_attr(
        feature = "codegen",
        schemars(with = "std::collections::HashMap<String, serde_json::Value>")
    )]
    pub other_fields: std::collections::HashMap<String, serde_yaml_ng::Value>,
}
