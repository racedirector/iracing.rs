//! # Session Information Parsing
//!
//! This module handles parsing of iRacing's session information from shared memory YAML data.
//! The session info contains metadata about the current racing session including track details,
//! weather conditions, participant information, and session timing.
//!
//! Raw byte extraction and decoding live in `crate::yaml_utils`; this module
//! owns the typed serde model and its deserialization entry point.

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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::schema::session::types::*;
    use crate::test_utils::{find_git_repository_root, require_test_data_file};
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

    // Property tests for comprehensive validation
    proptest! {
        #[test]
        fn prop_yaml_preprocessing_preserves_structure(
            yaml_content in r"[a-zA-Z0-9: \n\-\._]+",
        ) {
            let decoded = DecodedSessionYaml::new(yaml_content.clone());
            let processed = decoded.sanitize();

            // Processing should not make content significantly shorter
            // (Allow slight variations due to line ending normalization)
            let len_diff = processed.len() as i32 - yaml_content.len() as i32;
            prop_assert!(len_diff >= -2, "Processed length: {}, Original length: {}, Diff: {}", processed.len(), yaml_content.len(), len_diff);
        }

        #[test]
        fn prop_memory_extraction_handles_various_inputs(
            offset in 0..1000i32,
            length in 1..1000i32,
            memory_size in 1000..10000usize,
        ) {
            let memory = vec![65u8; memory_size]; // Fill with 'A' characters

            let result = crate::yaml_utils::extract_yaml_from_memory(&memory, offset, length);

            if (offset as usize + length as usize) <= memory_size {
                // Should succeed if within bounds
                prop_assert!(result.is_ok());
            } else {
                // Should fail if out of bounds
                prop_assert!(result.is_err());
            }
        }
    }

    #[test]
    #[ignore = "Need to implement known test structures"]
    fn parses_real_iracing_yaml_snapshot() -> Result<()> {
        // Test with real YAML data captured from live iRacing

        let snapshot_path = require_test_data_file("live_session_snapshot.yml")?;

        let yaml_content = std::fs::read_to_string(&snapshot_path)
            .with_context(|| format!("Reading YAML snapshot from {}", snapshot_path.display()))?;

        println!(
            "Testing with real iRacing YAML snapshot ({} bytes)",
            yaml_content.len()
        );

        let preprocessed = crate::yaml_utils::preprocess_iracing_yaml(&yaml_content)
            .expect("Failed to preprocess YAML");

        let session_info: SessionInfo = serde_yaml_ng::from_str(&preprocessed)
            .context("Failed to parse YAML to SessionInfo")?;

        // Validate the parsed structure matches what we expect from real data
        assert_eq!(
            session_info.weekend_info.track_name,
            "watkinsglen 2021 fullcourse"
        );
        assert_eq!(session_info.weekend_info.track_display_name, "Watkins Glen");
        assert_eq!(session_info.weekend_info.track_id, Some(434));
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
        assert_eq!(driver_info.driver_user_id, Some(932438));

        let drivers = driver_info
            .drivers
            .as_ref()
            .expect("Should have drivers list");
        assert_eq!(drivers.len(), 1);
        assert_eq!(drivers[0].user_name, "Kevin A O Neill");
        assert_eq!(drivers[0].car_idx, 0);
        assert_eq!(drivers[0].car_number, Some("037".to_string()));

        println!("✅ Real YAML snapshot parsing test passed!");
        println!(
            "   Track: {} ({})",
            session_info.weekend_info.track_name, session_info.weekend_info.track_display_name
        );
        println!("   Drivers: {}", drivers.len());
        println!("   Sessions: {}", session_info.session_info.sessions.len());

        Ok(())
    }

    #[test]
    #[cfg(feature = "benchmark")]
    fn benchmark_session_info_parsing_performance() {
        use std::time::Instant;

        // Create realistic test YAML
        let test_yaml = r#"
 DriverInfo:
- CarIdx: 0
  UserName: John O'Connor
  AbbrevName: J O'Con
  TeamName: '"Fast & Furious" Racing Team'
  Initials: JO
  CarNumber: "42"
  CarClassShortName: GT3
  CarIdxPosition: 1
- CarIdx: 1
  UserName: Sarah Mitchell
  AbbrevName: S Mitch
  TeamName: Lightning McQueen Racing
  Initials: SM
  CarNumber: "7"
  CarClassShortName: GT3
  CarIdxPosition: 2
WeatherInfo:
AirTemp: 25.0
TrackTemp: 35.2
Humidity: 65
WeatherType: Clear
TrackInfo:
TrackName: Watkins Glen International
TrackDisplayName: Watkins Glen
TrackLength: 5.472 km
TrackTurns: 11
TrackSurface: Asphalt
SessionInfo:
SessionType: Race
SessionLaps: 50
SessionTime: 3600.0
SessionState: Racing
"#;

        // Warm up
        for _ in 0..10 {
            let _ = crate::yaml_utils::preprocess_iracing_yaml(test_yaml);
        }

        // Benchmark YAML preprocessing
        const NUM_ITERATIONS: usize = 1000;
        let start = Instant::now();

        for _ in 0..NUM_ITERATIONS {
            let _ = crate::yaml_utils::preprocess_iracing_yaml(test_yaml).unwrap();
        }

        let elapsed = start.elapsed();
        let avg_duration_nanos = elapsed.as_nanos() as f64 / NUM_ITERATIONS as f64;
        let avg_duration_micros = avg_duration_nanos / 1000.0;

        println!(
            "Session YAML preprocessing performance: avg {:.2}ns ({:.3}μs) per parse, {} iterations",
            avg_duration_nanos, avg_duration_micros, NUM_ITERATIONS
        );

        // Target: <10ms total parse time (10,000μs) - should be much faster for preprocessing alone
        assert!(
            avg_duration_nanos < 1_000_000.0, // <1ms for preprocessing
            "Session YAML preprocessing should be <1ms, got {:.2}ns",
            avg_duration_nanos
        );

        // Benchmark complete parsing pipeline
        let preprocessed = crate::yaml_utils::preprocess_iracing_yaml(test_yaml).unwrap();
        let start = Instant::now();

        for _ in 0..100 {
            // Fewer iterations for full parsing
            let _ = SessionInfo::parse(&preprocessed);
        }

        let elapsed = start.elapsed();
        let avg_full_parse_micros = elapsed.as_micros() as f64 / 100.0;

        println!(
            "Complete session parsing performance: avg {:.2}μs per parse, 100 iterations",
            avg_full_parse_micros
        );

        // Target: <10ms (10,000μs) total parse time including YAML deserialization
        assert!(
            avg_full_parse_micros < 10_000.0,
            "Complete session parsing should be <10ms, got {:.2}μs",
            avg_full_parse_micros
        );

        if avg_full_parse_micros < 1_000.0 {
            println!("✅ Excellent performance: session parsing is <1ms");
        } else {
            println!("⚠️  Performance acceptable but could be optimized further");
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

        let header = connection.header();

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
        let raw_yaml = connection
            .session_yaml_bytes()
            .expect("Failed to get session info from iRacing")
            .unwrap()
            .decode()
            .expect("Could not decode session YAML")
            .sanitize();

        let session_info =
            SessionInfo::parse_sanitized(&raw_yaml).expect("Failed to parse live session info");

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
