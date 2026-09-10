//! # Session Information Parsing
//!
//! This module handles parsing of iRacing's session information from shared memory YAML data.
//! The session info contains metadata about the current racing session including track details,
//! weather conditions, participant information, and session timing.
//!
//! ## Key Features
//!
//! - **YAML Compatibility**: Handles iRacing's invalid YAML with unescaped characters
//! - **Performance Caching**: Version-based caching prevents unnecessary re-parsing
//! - **Memory Safety**: Safe extraction from Windows shared memory with bounds checking
//! - **Type Safety**: Full serde integration with comprehensive Rust type mapping
//! - **Error Resilience**: Graceful handling of malformed YAML and missing session data
//!
//! ## iRacing YAML Compatibility
//!
//! iRacing outputs invalid YAML containing unescaped characters in driver names and file paths
//! that break standard YAML parsers. This module includes preprocessing to fix these issues:
//!
//! ```text
//! // Problematic iRacing YAML:
//! UserName: O'Connor, Mike
//! TeamName: "Fast & Furious" Racing
//!
//! // After preprocessing:
//! UserName: 'O''Connor, Mike'
//! TeamName: '"Fast & Furious" Racing'
//! ```
//!
//! See: [iRacing Forum Discussion](https://forums.iracing.com/discussion/comment/374646#Comment_374646)
//!
//! ## Performance Characteristics
//!
//! - **YAML Preprocessing**: ~30μs (well under 1ms target)
//! - **Complete Parsing**: ~56μs (well under 10ms target)
//! - **Caching**: Version-based caching eliminates parsing when session unchanged
//! - **Memory Usage**: Single-pass preprocessing with minimal allocations
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │           Session Info Pipeline             │
//! │                                             │
//! │  Shared Memory  ──► YAML Extract ──► Cache. │
//! │       │                   │            │    │
//! │       │                   ▼            ▼    │
//! │       │            Preprocess ──► Parse     │
//! │       │                   │            │    │
//! │       │                   ▼            ▼    │
//! │       └────────── Validate ──► SessionInfo  │
//! │                                             │
//! └─────────────────────────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

// Submodules
pub mod camera;
pub mod car_setup;
#[cfg(feature = "schema-discovery")]
pub mod discovery;
pub mod driver;
pub mod radio;
pub mod session_data;
pub mod timing;
pub mod weekend;

// Re-exports for backward compatibility
pub use camera::{Camera, CameraGroup, CameraInfo};
pub use car_setup::CarSetup;
#[cfg(feature = "schema-discovery")]
pub use discovery::{
    UnknownField, UnknownFieldType, collect_leaf_fields, value_to_example, value_to_type,
};
pub use driver::{Driver, DriverInfoData, DriverTire};
pub use radio::{Frequency, Radio, RadioInfo};
#[cfg(feature = "codegen")]
use schemars::JsonSchema;
pub use session_data::{QualifyResult, QualifyResultsInfo, Session, SessionInfoData};
pub use timing::{Sector, SplitTimeInfo};
pub use weekend::{TelemetryOptions, WeekendInfo, WeekendOptions};

use crate::{IRacingSDKError, IRacingSessionString, Result, SessionInfoBuffer};

/// Session information extracted and parsed from iRacing's YAML session data
/// This matches the actual structure that iRacing outputs
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
pub struct SessionInfo {
    /// Weekend and track information
    pub weekend_info: WeekendInfo,
    /// Session information and session list
    pub session_info: SessionInfoData,
    /// Radio information
    #[serde(default)]
    pub radio_info: Option<RadioInfo>,
    /// Driver information (single object with current driver + drivers list)
    #[serde(default)]
    pub driver_info: Option<DriverInfoData>,
    /// Split timing information
    #[serde(default)]
    pub split_time_info: Option<SplitTimeInfo>,
    /// Car setup information
    #[serde(default)]
    #[cfg_attr(feature = "codegen", schemars(with = "Option<serde_json::Value>"))]
    pub car_setup: Option<CarSetup>,
    /// Camera information
    #[serde(default)]
    pub camera_info: Option<CameraInfo>,
    /// Qualifying results information
    #[serde(default)]
    pub qualify_results_info: Option<QualifyResultsInfo>,
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

impl SessionInfo {
    /// Parse cleaned YAML into SessionInfo
    ///
    /// The YAML should already be preprocessed to fix iRacing's non-standard format.
    /// This is a simple deserialization - preprocessing happens at lower levels.
    pub fn parse(yaml: &str) -> crate::Result<Self> {
        serde_yaml_ng::from_str(yaml).map_err(|e| crate::IRacingSDKError::Parse {
            context: "SessionInfo deserialization".to_string(),
            details: e.to_string(),
        })
    }

    /// Collect all unknown fields from all nested structures
    ///
    /// This recursively walks the session info tree and collects any fields
    /// that were present in the YAML but not mapped to known struct fields.
    /// Returns a list of unknown fields with their JSON paths, types, and example values.
    ///
    /// Only available when the `schema-discovery` feature is enabled.
    #[cfg(feature = "schema-discovery")]
    pub fn collect_unknown_fields(&self) -> Vec<UnknownField> {
        let mut fields = Vec::new();

        // Collect from SessionInfo root (recursively traverse objects/arrays)
        for (key, value) in &self.unknown_fields {
            fields.extend(collect_leaf_fields(key, value));
        }

        // Collect from WeekendInfo (recursively traverse objects/arrays)
        for (key, value) in &self.weekend_info.unknown_fields {
            let base_path = format!("WeekendInfo.{}", key);
            fields.extend(collect_leaf_fields(&base_path, value));
        }

        // Collect from WeekendInfo.TelemetryOptions (recursively traverse objects/arrays)
        if let Some(ref telemetry_options) = self.weekend_info.telemetry_options {
            for (key, value) in &telemetry_options.unknown_fields {
                let base_path = format!("WeekendInfo.TelemetryOptions.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }
        }

        // Collect from WeekendInfo.WeekendOptions (recursively traverse objects/arrays)
        if let Some(ref weekend_options) = self.weekend_info.weekend_options {
            for (key, value) in &weekend_options.unknown_fields {
                let base_path = format!("WeekendInfo.WeekendOptions.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }
        }

        // Collect from SessionInfo (recursively traverse objects/arrays)
        for (key, value) in &self.session_info.unknown_fields {
            let base_path = format!("SessionInfo.{}", key);
            fields.extend(collect_leaf_fields(&base_path, value));
        }

        // Collect from Sessions (recursively traverse objects/arrays)
        for (i, session) in self.session_info.sessions.iter().enumerate() {
            for (key, value) in &session.unknown_fields {
                let base_path = format!("SessionInfo.Sessions[{}].{}", i, key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }
        }

        // Collect from RadioInfo (recursively traverse objects/arrays)
        if let Some(ref radio_info) = self.radio_info {
            for (key, value) in &radio_info.unknown_fields {
                let base_path = format!("RadioInfo.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }

            if let Some(ref radios) = radio_info.radios {
                for (i, radio) in radios.iter().enumerate() {
                    for (key, value) in &radio.unknown_fields {
                        let base_path = format!("RadioInfo.Radios[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }

                    if let Some(ref frequencies) = radio.frequencies {
                        for (j, frequency) in frequencies.iter().enumerate() {
                            for (key, value) in &frequency.unknown_fields {
                                let base_path =
                                    format!("RadioInfo.Radios[{}].Frequencies[{}].{}", i, j, key);
                                fields.extend(collect_leaf_fields(&base_path, value));
                            }
                        }
                    }
                }
            }
        }

        // Collect from DriverInfo (recursively traverse objects/arrays)
        if let Some(ref driver_info) = self.driver_info {
            for (key, value) in &driver_info.unknown_fields {
                let base_path = format!("DriverInfo.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }

            if let Some(ref tires) = driver_info.driver_tires {
                for (i, tire) in tires.iter().enumerate() {
                    for (key, value) in &tire.unknown_fields {
                        let base_path = format!("DriverInfo.DriverTires[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }
                }
            }

            if let Some(ref drivers) = driver_info.drivers {
                for (i, driver) in drivers.iter().enumerate() {
                    for (key, value) in &driver.unknown_fields {
                        let base_path = format!("DriverInfo.Drivers[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }
                }
            }
        }

        // Collect from SplitTimeInfo (recursively traverse objects/arrays)
        if let Some(ref split_time_info) = self.split_time_info {
            for (key, value) in &split_time_info.unknown_fields {
                let base_path = format!("SplitTimeInfo.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }

            if let Some(ref sectors) = split_time_info.sectors {
                for (i, sector) in sectors.iter().enumerate() {
                    for (key, value) in &sector.unknown_fields {
                        let base_path = format!("SplitTimeInfo.Sectors[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }
                }
            }
        }

        // Collect from CameraInfo (recursively traverse objects/arrays)
        if let Some(ref camera_info) = self.camera_info {
            for (key, value) in &camera_info.unknown_fields {
                let base_path = format!("CameraInfo.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }

            if let Some(ref groups) = camera_info.groups {
                for (i, group) in groups.iter().enumerate() {
                    for (key, value) in &group.unknown_fields {
                        let base_path = format!("CameraInfo.Groups[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }

                    if let Some(ref cameras) = group.cameras {
                        for (j, camera) in cameras.iter().enumerate() {
                            for (key, value) in &camera.unknown_fields {
                                let base_path =
                                    format!("CameraInfo.Groups[{}].Cameras[{}].{}", i, j, key);
                                fields.extend(collect_leaf_fields(&base_path, value));
                            }
                        }
                    }
                }
            }
        }

        // Collect from QualifyResultsInfo (recursively traverse objects/arrays)
        if let Some(ref qualify_results_info) = self.qualify_results_info {
            for (key, value) in &qualify_results_info.unknown_fields {
                let base_path = format!("QualifyResultsInfo.{}", key);
                fields.extend(collect_leaf_fields(&base_path, value));
            }

            if let Some(ref results) = qualify_results_info.results {
                for (i, result) in results.iter().enumerate() {
                    for (key, value) in &result.unknown_fields {
                        let base_path = format!("QualifyResultsInfo.Results[{}].{}", i, key);
                        fields.extend(collect_leaf_fields(&base_path, value));
                    }
                }
            }
        }

        fields
    }
}

impl TryFrom<SessionInfoBuffer> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: SessionInfoBuffer) -> Result<Self, Self::Error> {
        let session_info = IRacingSessionString::try_from(value)?;
        SessionInfo::try_from(session_info)
    }
}

impl TryFrom<IRacingSessionString> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: IRacingSessionString) -> Result<Self> {
        let session_info = String::from(value);
        Ok(SessionInfo::parse(&session_info)?)
    }
}

#[cfg(test)]
mod tests;
