//! # Session Information Parsing
//!
//! This module handles parsing of iRacing's session information from shared memory YAML data.
//! The session info contains metadata about the current racing session including track details,
//! weather conditions, participant information, and session timing.
//!
//! Raw byte extraction, decoding, and sanitation live in [`types`]. This module
//! owns the typed serde model and its deserialization entry points.

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
pub mod types;
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

use crate::schema::session::types::{DecodedSessionYaml, SanitizedSessionYaml};

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
    /// Derserialize iRacing session YAML
    pub fn parse_sanitized(yaml: &SanitizedSessionYaml) -> crate::Result<Self> {
        serde_yaml_ng::from_str(&yaml).map_err(crate::IRacingSDKError::session_yaml_deserialization)
    }

    /// Preprocess and deserialize iRacing session YAML.
    pub fn parse(yaml: &str) -> crate::Result<Self> {
        let yaml = DecodedSessionYaml::new(yaml.to_owned()).sanitize();
        Self::parse_sanitized(&yaml)
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

#[cfg(test)]
mod tests {
    use super::{SanitizedSessionYaml, SessionInfo};
    use crate::IRacingSDKError;

    const MINIMAL_SESSION: &str = r#"WeekendInfo:
  TrackName: test
  TrackDisplayName: Test Circuit
  TrackLength: "1.00 km"
SessionInfo:
  CurrentSessionNum: 0
  Sessions:
    - SessionNum: 0
      SessionLaps: unlimited
      SessionTime: "600 sec"
      SessionType: Practice
"#;

    fn parse_context(error: IRacingSDKError) -> String {
        match error {
            IRacingSDKError::Parse { context, .. } => context,
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn parse_deserializes_decoded_session_yaml() {
        let session = SessionInfo::parse(MINIMAL_SESSION).unwrap();

        assert_eq!(session.weekend_info.track_name, "test");
        assert_eq!(session.weekend_info.track_display_name, "Test Circuit");
        assert_eq!(session.session_info.sessions.len(), 1);
    }

    #[test]
    fn parse_and_parse_sanitized_are_equivalent_for_clean_yaml() {
        let sanitized = SanitizedSessionYaml::new(MINIMAL_SESSION.to_owned());

        assert_eq!(
            SessionInfo::parse(MINIMAL_SESSION).unwrap(),
            SessionInfo::parse_sanitized(&sanitized).unwrap()
        );
    }

    #[test]
    fn parse_sanitizes_decoded_control_characters() {
        let yaml = MINIMAL_SESSION.replace("TrackName: test", "TrackName: te\u{1}st");
        let session = SessionInfo::parse(&yaml).unwrap();

        assert_eq!(session.weekend_info.track_name, "test");
    }

    #[test]
    fn malformed_yaml_reports_deserialization_context() {
        let error = SessionInfo::parse("WeekendInfo: [").unwrap_err();

        assert_eq!(parse_context(error), "session YAML deserialization");
    }

    #[test]
    fn type_mismatch_reports_deserialization_context() {
        let yaml = MINIMAL_SESSION.replace("CurrentSessionNum: 0", "CurrentSessionNum: nope");
        let error = SessionInfo::parse(&yaml).unwrap_err();

        assert_eq!(parse_context(error), "session YAML deserialization");
    }

    #[test]
    fn missing_required_top_level_sections_are_rejected() {
        for yaml in [
            "SessionInfo:\n  CurrentSessionNum: 0\n",
            "WeekendInfo:\n  TrackName: test\n",
            "{}",
        ] {
            let error = SessionInfo::parse(yaml).unwrap_err();
            assert_eq!(parse_context(error), "session YAML deserialization");
        }
    }

    #[test]
    fn empty_or_controls_only_input_is_rejected() {
        for yaml in ["", "\u{0}\u{1}\u{7f}"] {
            let error = SessionInfo::parse(yaml).unwrap_err();
            assert_eq!(parse_context(error), "session YAML deserialization");
        }
    }
}
