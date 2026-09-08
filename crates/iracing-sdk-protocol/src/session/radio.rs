//! Radio and frequency information
//!
//! This module contains radio communication structures including radio
//! configurations and frequency details.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

/// Radio information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct RadioInfo {
    /// Currently selected radio number
    pub selected_radio_num: Option<i32>,
    /// List of radios
    pub radios: Option<Vec<Radio>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(
        feature = "codegen",
        schemars(with = "std::collections::HashMap<String, serde_json::Value>")
    )]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Individual radio configuration
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct Radio {
    /// Radio number
    pub radio_num: Option<i32>,
    /// Hop count
    pub hop_count: Option<i32>,
    /// Number of frequencies
    pub num_frequencies: Option<i32>,
    /// Currently tuned frequency number
    pub tuned_to_frequency_num: Option<i32>,
    /// Scanning enabled flag
    pub scanning_is_on: Option<i32>,
    /// List of frequencies
    pub frequencies: Option<Vec<Frequency>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(
        feature = "codegen",
        schemars(with = "std::collections::HashMap<String, serde_json::Value>")
    )]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Radio frequency configuration
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct Frequency {
    /// Frequency number
    pub frequency_num: Option<i32>,
    /// Frequency name
    pub frequency_name: Option<String>,
    /// Priority level
    pub priority: Option<i32>,
    /// Car index (-1 for broadcast channels)
    pub car_idx: Option<i32>,
    /// Entry index
    pub entry_idx: Option<i32>,
    /// Club ID
    #[serde(rename = "ClubID")]
    pub club_id: Option<i32>,
    /// Can scan flag
    pub can_scan: Option<i32>,
    /// Can squawk flag
    pub can_squawk: Option<i32>,
    /// Muted flag
    pub muted: Option<i32>,
    /// Is mutable flag
    pub is_mutable: Option<i32>,
    /// Is deletable flag
    pub is_deletable: Option<i32>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(
        feature = "codegen",
        schemars(with = "std::collections::HashMap<String, serde_json::Value>")
    )]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}
