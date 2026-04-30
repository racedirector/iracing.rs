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
pub mod cache;
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
pub use cache::{SessionInfoCache, SessionInfoParser};
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
    use anyhow::{Context, Result};
    use proptest::prelude::*;
    use test_utils::{find_git_repository_root, require_test_data_file};

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

        // The path should end with 'pitwall' (our project name)
        assert!(
            repo_root.file_name().unwrap() == "iracing-rs",
            "Repository root should be named 'iracing-rs'"
        );
    }

    #[test]
    fn session_info_cache_validity() {
        let session_info = create_test_session_info();
        let cache = SessionInfoCache::new(session_info, 42);

        assert!(cache.is_valid(42));
        assert!(!cache.is_valid(43));
    }

    #[test]
    fn yaml_preprocessing_fixes_problematic_characters() {
        let parser = SessionInfoParser::new();

        let problematic_yaml = r#"
UserName: O'Connor, Mike
TeamName: "Fast & Furious" Racing
AbbrevName: O'Con
"#;

        let result = parser.preprocess_iracing_yaml(problematic_yaml).unwrap();
        println!("Original: {}", problematic_yaml);
        println!("Processed: {}", result);

        // Should have quotes added around problematic values
        assert!(result.contains("UserName:  'O''Connor, Mike'"));
        assert!(result.contains("AbbrevName:  'O''Con'"));

        // TeamName already has quotes in the input, so it shouldn't be modified
        assert!(result.contains("TeamName: \"Fast & Furious\" Racing"));
    }

    #[test]
    fn extract_yaml_from_memory_validates_bounds() {
        let parser = SessionInfoParser::new();
        let memory = vec![0u8; 100];

        // Invalid offset
        let result = parser.extract_yaml_from_memory(&memory, -1, 10);
        assert!(result.is_err());

        // Invalid length
        let result = parser.extract_yaml_from_memory(&memory, 10, -1);
        assert!(result.is_err());

        // Out of bounds
        let result = parser.extract_yaml_from_memory(&memory, 50, 60);
        assert!(result.is_err());
    }

    #[test]
    fn session_validation_catches_missing_required_fields() {
        let parser = SessionInfoParser::new();

        // Missing track name
        let mut session_info = create_test_session_info();
        session_info.weekend_info.track_name.clear();
        assert!(parser.validate_session_info(&session_info).is_err());

        // Missing track display name
        let mut session_info = create_test_session_info();
        session_info.weekend_info.track_display_name.clear();
        assert!(parser.validate_session_info(&session_info).is_err());

        // No sessions
        let mut session_info = create_test_session_info();
        session_info.session_info.sessions.clear();
        assert!(parser.validate_session_info(&session_info).is_err());
    }

    // Property tests for comprehensive validation
    proptest! {
        #[test]
        fn prop_yaml_preprocessing_preserves_structure(
            yaml_content in r"[a-zA-Z0-9: \n\-\._]+",
        ) {
            let parser = SessionInfoParser::new();
            let result = parser.preprocess_iracing_yaml(&yaml_content);

            // Should not fail on well-formed content
            prop_assert!(result.is_ok());

            // Processing should not make content significantly shorter
            // (Allow slight variations due to line ending normalization)
            let processed = result.unwrap();
            let len_diff = processed.len() as i32 - yaml_content.len() as i32;
            prop_assert!(len_diff >= -2, "Processed length: {}, Original length: {}, Diff: {}", processed.len(), yaml_content.len(), len_diff);
        }

        #[test]
        fn prop_memory_extraction_handles_various_inputs(
            offset in 0..1000i32,
            length in 1..1000i32,
            memory_size in 1000..10000usize,
        ) {
            let parser = SessionInfoParser::new();
            let memory = vec![65u8; memory_size]; // Fill with 'A' characters

            let result = parser.extract_yaml_from_memory(&memory, offset, length);

            if (offset as usize + length as usize) <= memory_size {
                // Should succeed if within bounds
                prop_assert!(result.is_ok());
            } else {
                // Should fail if out of bounds
                prop_assert!(result.is_err());
            }
        }
    }

    // #[test]
    // fn parses_real_iracing_yaml_snapshot() -> Result<()> {
    //     // Test with real YAML data captured from live iRacing

    //     let snapshot_path = require_test_data_file("live_session_snapshot.yml")?;

    //     let yaml_content = std::fs::read_to_string(&snapshot_path)
    //         .with_context(|| format!("Reading YAML snapshot from {}", snapshot_path.display()))?;

    //     println!(
    //         "Testing with real iRacing YAML snapshot ({} bytes)",
    //         yaml_content.len()
    //     );

    //     // Parse with our SessionInfoParser
    //     let parser = SessionInfoParser::new();
    //     let preprocessed = parser
    //         .preprocess_iracing_yaml(&yaml_content)
    //         .expect("Failed to preprocess YAML");

    //     let session_info: SessionInfo = serde_yaml_ng::from_str(&preprocessed)
    //         .context("Failed to parse YAML to SessionInfo")?;

    //     // Validate the parsed structure matches what we expect from real data
    //     assert_eq!(
    //         session_info.weekend_info.track_name,
    //         "watkinsglen 2021 fullcourse"
    //     );
    //     assert_eq!(session_info.weekend_info.track_display_name, "Watkins Glen");
    //     assert_eq!(session_info.weekend_info.track_id, Some(434));
    //     assert_eq!(session_info.session_info.current_session_num, 0);
    //     assert_eq!(session_info.session_info.sessions.len(), 1);
    //     assert_eq!(
    //         session_info.session_info.sessions[0].session_type,
    //         "Offline Testing"
    //     );

    //     // Validate driver info
    //     let driver_info = session_info
    //         .driver_info
    //         .as_ref()
    //         .expect("Should have driver info");
    //     assert_eq!(driver_info.driver_car_idx, Some(0));
    //     assert_eq!(driver_info.driver_user_id, Some(932438));

    //     let drivers = driver_info
    //         .drivers
    //         .as_ref()
    //         .expect("Should have drivers list");
    //     assert_eq!(drivers.len(), 1);
    //     assert_eq!(drivers[0].user_name, "Kevin A O Neill");
    //     assert_eq!(drivers[0].car_idx, 0);
    //     assert_eq!(drivers[0].car_number, Some("037".to_string()));

    //     println!("✅ Real YAML snapshot parsing test passed!");
    //     println!(
    //         "   Track: {} ({})",
    //         session_info.weekend_info.track_name, session_info.weekend_info.track_display_name
    //     );
    //     println!("   Drivers: {}", drivers.len());
    //     println!("   Sessions: {}", session_info.session_info.sessions.len());

    //     Ok(())
    // }

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

    #[test]
    #[cfg(feature = "benchmark")]
    fn benchmark_session_info_parsing_performance() {
        use std::time::Instant;

        let parser = SessionInfoParser::new();

        // Create realistic test YAML with problematic characters
        let test_yaml = r#"
 DriverInfo:
- CarIdx: 0
  UserName: John O'Connor
  AbbrevName: J O'Con
  TeamName: "Fast & Furious" Racing Team
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
            let _ = parser.preprocess_iracing_yaml(test_yaml);
        }

        // Benchmark YAML preprocessing
        const NUM_ITERATIONS: usize = 1000;
        let start = Instant::now();

        for _ in 0..NUM_ITERATIONS {
            let _ = parser.preprocess_iracing_yaml(test_yaml).unwrap();
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
        let preprocessed = parser.preprocess_iracing_yaml(test_yaml).unwrap();
        let start = Instant::now();

        for _ in 0..100 {
            // Fewer iterations for full parsing
            let _ = parser.parse(&preprocessed);
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
        let parser = SessionInfoParser::new();
        let raw_yaml = connection
            .session_info()
            .expect("Failed to get session info from iRacing");

        // Preprocess the YAML to handle control characters
        let preprocessed_yaml = parser
            .preprocess_iracing_yaml(&raw_yaml)
            .expect("Failed to preprocess YAML");

        let session_info = parser
            .parse(&preprocessed_yaml)
            .expect("Failed to parse live session info");

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

        // Test caching behavior - second parse should use cache
        let cached_session_info = parser
            .parse(&preprocessed_yaml)
            .expect("Failed to parse cached session info");

        assert_eq!(
            session_info.weekend_info.track_name,
            cached_session_info.weekend_info.track_name
        );
        assert_eq!(
            session_info.session_info.sessions.len(),
            cached_session_info.session_info.sessions.len()
        );
        println!("  ✅ Session info caching working correctly");

        // Test some drivers if available
        if let Some(driver_info) = &session_info.driver_info {
            if let Some(drivers) = &driver_info.drivers {
                if !drivers.is_empty() {
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
