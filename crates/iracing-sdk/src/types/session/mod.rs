//! Typed representations of iRacing session information.
//!
//! [`SessionInfo`] and its submodules model the cleaned YAML published by live
//! telemetry and embedded in IBT recordings. Acquisition, byte decoding, YAML
//! cleanup, version tracking, and publication policy live outside this module;
//! callers should preprocess raw text with [`crate::yaml_utils`] before calling
//! [`SessionInfo::parse`].
//!
//! The `schema-discovery` feature retains unknown fields so schema-generation
//! tools can report simulator additions without weakening the typed model.

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

// Typed session model exports.
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

use crate::{IRacingSDKError, Result, SessionInfoBuffer, irsdk::IRacingSessionString};

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

impl TryFrom<&str> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: &str) -> Result<Self> {
        serde_yaml_ng::from_str(value).map_err(|e| crate::IRacingSDKError::Parse {
            context: "SessionInfo deserialization".to_string(),
            details: e.to_string(),
        })
    }
}

impl TryFrom<String> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: String) -> Result<Self> {
        SessionInfo::try_from(value.as_ref())
    }
}

impl TryFrom<IRacingSessionString> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: IRacingSessionString) -> Result<Self> {
        SessionInfo::try_from(value.as_ref())
    }
}

impl TryFrom<SessionInfoBuffer> for SessionInfo {
    type Error = IRacingSDKError;

    fn try_from(value: SessionInfoBuffer) -> Result<Self> {
        let yaml = IRacingSessionString::try_from(value)?;
        SessionInfo::try_from(yaml)
    }
}

impl TryFrom<SessionInfo> for String {
    type Error = IRacingSDKError;

    fn try_from(value: SessionInfo) -> Result<Self> {
        serde_yaml_ng::to_string(&value).map_err(|e| crate::IRacingSDKError::Parse {
            context: "SessionInfo serialization".to_string(),
            details: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::find_git_repository_root;
    #[cfg(windows)]
    use crate::test_utils::require_test_data_file;
    #[cfg(windows)]
    use anyhow::{Context, Result};
    use proptest::prelude::*;

    #[test]
    fn find_git_repository_root_works() {
        // Test that we can find the git repository root
        let repo_root = find_git_repository_root().expect("Should find git repository root");

        // Verify it contains a .git directory
        assert!(
            repo_root.join(".git").exists(),
            "Repository root should contain .git directory"
        );

        // Verify it contains expected project files (Cargo.toml should be at workspace root)
        assert!(
            repo_root.join("Cargo.toml").exists(),
            "Repository root should contain Cargo.toml"
        );

        println!("Found git repository root: {:?}", repo_root);

        let repo_path_name = repo_root.file_name().unwrap();

        // The path should end with 'iracing.rs' (our project name)
        assert!(
            repo_path_name == "iracing.rs",
            "Repository root should be named 'iracing.rs'. Received: {:?}",
            repo_path_name
        );
    }

    #[test]
    fn yaml_preprocessing_preserves_printable_characters() {
        let yaml = r#"
UserName: O'Connor, Mike
TeamName: "Fast & Furious" Racing
AbbrevName: O'Con
"#;

        // Parse the string into a sanitized session string
        let result: String = IRacingSessionString::try_from(yaml).unwrap().into();

        assert_eq!(result, yaml);
    }

    // Property tests for comprehensive validation
    proptest! {
        #[test]
        fn prop_yaml_preprocessing_preserves_structure(
            yaml_content in r"[a-zA-Z0-9: \n\-\._]+",
        ) {
            let result = IRacingSessionString::try_from(yaml_content.as_str());

            if yaml_content.trim().is_empty() {
                prop_assert!(result.is_err());
            } else {
                let sanitized = result.unwrap();
                prop_assert_eq!(sanitized.as_ref(), yaml_content.as_str());
            }
        }
    }

    #[test]
    fn session_info_buffer_conversion_decodes_sanitizes_and_parses() {
        let mut bytes = vec![0x01];
        bytes.extend_from_slice(
            br#"WeekendInfo:
  TrackName: test-track
  TrackLength: 1.0 km
  TrackDisplayName: Test Track
SessionInfo:
  CurrentSessionNum: 0
  Sessions: []
"#,
        );
        bytes.extend_from_slice(b"\0ignored padding");

        let session_info = SessionInfo::try_from(SessionInfoBuffer::from_snapshot(bytes))
            .expect("the SDK buffer should decode, sanitize, and deserialize");

        assert_eq!(session_info.weekend_info.track_name, "test-track");
        assert_eq!(session_info.weekend_info.track_display_name, "Test Track");
        assert_eq!(session_info.session_info.current_session_num, 0);
        assert!(session_info.session_info.sessions.is_empty());
    }

    #[test]
    fn session_info_buffer_conversion_rejects_content_empty_after_sanitizing() {
        let buffer = SessionInfoBuffer::from_snapshot(b"\x01\x02\x03\0padding".to_vec());

        let error = SessionInfo::try_from(buffer)
            .expect_err("control-character-only session data should be rejected");

        assert!(matches!(
            error,
            IRacingSDKError::Parse { context, .. } if context == "YAML preprocessing"
        ));
    }

    #[test]
    fn session_info_buffer_conversion_reports_malformed_yaml() {
        let buffer = SessionInfoBuffer::from_snapshot(b"WeekendInfo: [\0padding".to_vec());

        let error =
            SessionInfo::try_from(buffer).expect_err("malformed session YAML should be rejected");

        assert!(matches!(
            error,
            IRacingSDKError::Parse { context, .. } if context == "SessionInfo deserialization"
        ));
    }

    #[test]
    fn parses_checked_in_live_session_snapshot() -> Result<()> {
        let snapshot_path = require_test_data_file("live-session-snapshot.yml")?;

        let yaml_content = std::fs::read(&snapshot_path)
            .with_context(|| format!("Reading YAML snapshot from {}", snapshot_path.display()))?;

        println!(
            "Testing with real iRacing YAML snapshot ({} bytes)",
            yaml_content.len()
        );

        let session_info = SessionInfo::try_from(SessionInfoBuffer::from_snapshot(yaml_content))
            .context("Failed to convert session buffer to SessionInfo")?;

        // Validate the parsed structure matches the checked-in capture.
        assert_eq!(session_info.weekend_info.track_name, "roadamerica full");
        assert_eq!(session_info.weekend_info.track_display_name, "Road America");
        assert_eq!(session_info.weekend_info.track_id, Some(18));
        assert_eq!(session_info.session_info.current_session_num, 0);
        assert_eq!(session_info.session_info.sessions.len(), 1);
        assert_eq!(
            session_info.session_info.sessions[0].session_type,
            "Offline Testing"
        );

        // Validate driver info
        let driver_info = session_info
            .driver_info
            .as_ref()
            .expect("Should have driver info");
        assert_eq!(driver_info.driver_car_idx, Some(0));
        assert_eq!(driver_info.driver_user_id, Some(378767));

        let drivers = driver_info
            .drivers
            .as_ref()
            .expect("Should have drivers list");
        assert_eq!(drivers.len(), 1);
        assert_eq!(drivers[0].user_name, "Justin Makaila");
        assert_eq!(drivers[0].car_idx, 0);
        assert_eq!(drivers[0].car_number, Some("64".to_string()));

        println!("✅ Real YAML snapshot parsing test passed!");
        println!(
            "   Track: {} ({})",
            session_info.weekend_info.track_name, session_info.weekend_info.track_display_name
        );
        println!("   Drivers: {}", drivers.len());
        println!("   Sessions: {}", session_info.session_info.sessions.len());

        Ok(())
    }

    #[allow(dead_code)]
    fn create_test_session_info() -> SessionInfo {
        SessionInfo {
            weekend_info: WeekendInfo {
                track_name: "bathurst".to_string(),
                track_id: Some(219),
                track_length: "6.1441 km".to_string(),
                track_length_official: Some("6.21 km".to_string()),
                track_display_name: "Mount Panorama Circuit".to_string(),
                track_display_short_name: Some("Bathurst".to_string()),
                track_config_name: Some("".to_string()),
                track_city: Some("Bathurst".to_string()),
                track_state: Some("New South Wales".to_string()),
                track_country: Some("Australia".to_string()),
                track_altitude: Some("708.99 m".to_string()),
                track_num_turns: Some(23),
                track_type: Some("road course".to_string()),
                track_surface_temp: Some("35.69 C".to_string()),
                track_air_temp: Some("20.69 C".to_string()),
                track_wind_vel: Some("4.33 m/s".to_string()),
                track_wind_dir: Some("4.19 rad".to_string()),
                track_relative_humidity: Some("31 %".to_string()),
                event_type: Some("Test".to_string()),
                category: Some("Road".to_string()),
                build_version: Some("2025.09.09.01".to_string()),
                ..Default::default()
            },
            session_info: SessionInfoData {
                current_session_num: 0,
                sessions: vec![Session {
                    session_num: 0,
                    session_laps: "unlimited".to_string(),
                    session_time: "unlimited".to_string(),
                    session_type: "Offline Testing".to_string(),
                    session_name: Some("TESTING".to_string()),
                    session_track_rubber_state: Some("moderately low usage".to_string()),
                    session_sub_type: Some("".to_string()),
                    session_skipped: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            },
            radio_info: None,
            driver_info: Some(DriverInfoData {
                driver_car_idx: Some(0),
                driver_user_id: Some(932438),
                pace_car_idx: Some(-1),
                driver_is_admin: Some(1),
                driver_setup_name: Some("Test Setup".to_string()),
                drivers: Some(vec![Driver {
                    car_idx: 0,
                    user_name: "Test Driver".to_string(),
                    abbrev_name: Some("".to_string()),
                    initials: Some("".to_string()),
                    user_id: Some(932438),
                    team_id: Some(0),
                    team_name: Some("Test Team".to_string()),
                    car_number: Some("037".to_string()),
                    car_screen_name: Some("Test Car".to_string()),
                    car_is_pace_car: Some(0),
                    car_is_ai: Some(0),
                    i_rating: Some(1),
                    lic_level: Some(1),
                    is_spectator: Some(0),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            split_time_info: None,
            car_setup: None,
            camera_info: None,
            qualify_results_info: None,
            #[cfg(feature = "schema-discovery")]
            unknown_fields: HashMap::new(),
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "iracing_required"]
    fn parses_live_iracing_session_info() {
        use crate::windows::Connection;

        // Open connection to live iRacing shared memory
        let connection = Connection::try_connect()
            .expect("Failed to connect to iRacing - ensure iRacing is running and in a session");

        let header = connection.header_snapshot();

        println!("Live iRacing header info:");
        println!("  Session info length: {} bytes", header.session_info_len);
        println!("  Session info offset: {}", header.session_info_offset);
        println!(
            "  Session info update counter: {}",
            header.session_info_update
        );

        // Validate we have session info
        assert!(header.session_info_len > 0, "No session info available");
        assert!(
            header.session_info_offset >= 0,
            "Invalid session info offset"
        );

        // Get and parse session info
        let session_buffer = connection
            .session_info_buffer()
            .expect("Failed to get session info from iRacing");
        let reparsing_buffer = session_buffer.clone();
        let session_info = SessionInfo::try_from(session_buffer)
            .expect("Failed to convert live session buffer to SessionInfo");

        // Validate session info content
        println!("\nLive session info parsed successfully:");
        println!(
            "  Track: {} ({})",
            session_info.weekend_info.track_name, session_info.weekend_info.track_display_name
        );
        println!("  Track length: {}", session_info.weekend_info.track_length);
        println!(
            "  Current session: {}",
            session_info.session_info.current_session_num
        );
        if !session_info.session_info.sessions.is_empty() {
            println!(
                "  Session type: {}",
                session_info.session_info.sessions[0].session_type
            );
        }
        println!(
            "  Number of sessions: {}",
            session_info.session_info.sessions.len()
        );
        if let Some(driver_info) = &session_info.driver_info {
            if let Some(drivers) = &driver_info.drivers {
                println!("  Number of drivers: {}", drivers.len());
            } else {
                println!("  No drivers list available");
            }
            if let Some(current_driver) = driver_info.driver_car_idx {
                println!("  Current driver car index: {}", current_driver);
            }
        } else {
            println!("  No driver info available (testing session)");
        }

        // Basic validation
        assert!(
            !session_info.weekend_info.track_name.is_empty(),
            "Track name should not be empty"
        );
        assert!(
            !session_info.weekend_info.track_display_name.is_empty(),
            "Track display name should not be empty"
        );
        assert!(
            !session_info.session_info.sessions.is_empty(),
            "Should have at least one session"
        );

        let reparsed_session_info = SessionInfo::try_from(reparsing_buffer)
            .expect("Failed to reconvert live session buffer to SessionInfo");

        assert_eq!(
            session_info.weekend_info.track_name,
            reparsed_session_info.weekend_info.track_name
        );
        assert_eq!(
            session_info.session_info.sessions.len(),
            reparsed_session_info.session_info.sessions.len()
        );
        println!("  ✅ Session info reparsing is deterministic");

        // Test some drivers if available
        if let Some(driver_info) = &session_info.driver_info
            && let Some(drivers) = &driver_info.drivers
            && !drivers.is_empty()
        {
            println!("\nDriver information:");
            for (i, driver) in drivers.iter().take(3).enumerate() {
                println!(
                    "  Driver {}: {} ({})",
                    i + 1,
                    driver.user_name,
                    driver.abbrev_name.as_deref().unwrap_or("N/A")
                );
            }
        }

        // Test weather info if available
        println!("\nWeather information:");
        if let Some(air_temp) = &session_info.weekend_info.track_air_temp {
            println!("  Air temperature: {}", air_temp);
        }
        if let Some(surface_temp) = &session_info.weekend_info.track_surface_temp {
            println!("  Track surface temperature: {}", surface_temp);
        }
        if let Some(humidity) = &session_info.weekend_info.track_relative_humidity {
            println!("  Relative humidity: {}", humidity);
        }

        println!("\n✅ Live session info parsing test completed successfully");
    }
}
