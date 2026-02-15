//! Camera configuration structures
//!
//! This module contains camera-related information including camera groups
//! and individual camera configurations.

use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

/// Camera information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct CameraInfo {
    /// Camera groups
    pub groups: Option<Vec<CameraGroup>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Camera group information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct CameraGroup {
    /// Group number
    pub group_num: Option<i32>,
    /// Group name
    pub group_name: Option<String>,
    /// Whether this is a scenic camera group
    pub is_scenic: Option<bool>,
    /// List of cameras in this group
    pub cameras: Option<Vec<Camera>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Individual camera information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct Camera {
    /// Camera number
    pub camera_num: Option<i32>,
    /// Camera name
    pub camera_name: Option<String>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}
