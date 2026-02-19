//! Session data structures
//!
//! This module contains session-related data structures including session lists
//! and individual session information.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

/// Session information data from iRacing
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct SessionInfoData {
    /// Current session number
    pub current_session_num: i32,
    /// List of sessions
    pub sessions: Vec<Session>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Individual session data
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct Session {
    /// Session number
    pub session_num: i32,
    /// Session laps ("unlimited" or number)
    pub session_laps: String,
    /// Session time ("unlimited" or time)
    pub session_time: String,
    /// Number of laps to average for qualifying
    pub session_num_laps_to_avg: Option<i32>,
    /// Session type
    pub session_type: String,
    /// Session name
    pub session_name: Option<String>,
    /// Session track rubber state
    pub session_track_rubber_state: Option<String>,
    /// Session sub type
    pub session_sub_type: Option<String>,
    /// Whether session was skipped
    pub session_skipped: Option<i32>,
    /// Whether run groups were used
    pub session_run_groups_used: Option<i32>,
    /// Whether tire compound change is enforced
    pub session_enforce_tire_compound_change: Option<i32>,
    /// Results positions
    #[cfg_attr(feature = "codegen", schemars(with = "Option<Vec<serde_json::Value>>"))]
    pub results_positions: Option<Vec<serde_yaml_ng::Value>>,
    /// Results fastest lap data
    #[cfg_attr(feature = "codegen", schemars(with = "Option<Vec<serde_json::Value>>"))]
    pub results_fastest_lap: Option<Vec<serde_yaml_ng::Value>>,
    /// Results average lap time
    pub results_average_lap_time: Option<f64>,
    /// Number of caution flags
    pub results_num_caution_flags: Option<i32>,
    /// Number of caution laps
    pub results_num_caution_laps: Option<i32>,
    /// Number of lead changes
    pub results_num_lead_changes: Option<i32>,
    /// Laps complete
    pub results_laps_complete: Option<i32>,
    /// Whether results are official
    pub results_official: Option<i32>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Qualifying results information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct QualifyResultsInfo {
    /// List of qualifying results
    pub results: Option<Vec<QualifyResult>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Individual qualifying result
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct QualifyResult {
    /// Overall position
    pub position: Option<i32>,
    /// Class position
    pub class_position: Option<i32>,
    /// Car index
    pub car_idx: Option<i32>,
    /// Fastest lap number
    pub fastest_lap: Option<i32>,
    /// Fastest lap time
    pub fastest_time: Option<f64>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}
