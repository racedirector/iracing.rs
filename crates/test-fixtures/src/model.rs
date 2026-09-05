//! In-memory fixture profiles and the schema-version-1 manifest model.
//!
//! Profiles are generator inputs. Manifest types mirror the checked-in JSON
//! contract consumed both here and by `iracing_sdk::test_utils`.

use iracing_sdk::irsdk::VariableType;
use serde::{Deserialize, Serialize};

/// One telemetry variable declared by a generated profile.
#[derive(Debug, Clone)]
pub(crate) struct Variable {
    /// SDK variable name.
    pub name: &'static str,
    /// SDK storage type written into its variable header.
    pub data_type: VariableType,
    /// Byte offset within a telemetry frame.
    pub offset: i32,
    /// Number of adjacent values stored for the variable.
    pub count: i32,
    /// SDK unit string.
    pub units: &'static str,
    /// Human-readable SDK variable description.
    pub description: &'static str,
    /// Whether iRacing interprets `count` as a time dimension.
    pub count_as_time: bool,
}

/// Complete input definition for one deterministic IBT fixture.
#[derive(Debug, Clone)]
pub(crate) struct Profile {
    /// Stable profile and output-file stem.
    pub name: &'static str,
    /// Seed passed to the explicitly selected deterministic RNG.
    pub seed: u64,
    /// Machine-oriented track name embedded in session YAML.
    pub track_name: &'static str,
    /// Display track name embedded in session YAML.
    pub track_display_name: &'static str,
    /// Session type and display name embedded in session YAML.
    pub session_name: &'static str,
    /// Number of telemetry frames per second.
    pub tick_rate: i32,
    /// Number of fixed-size telemetry frames to generate.
    pub frame_count: usize,
    /// Size in bytes of every telemetry frame.
    pub frame_size: usize,
    /// Completed-lap count recorded in the disk sub-header.
    pub lap_count: i32,
    /// Unix timestamp recorded in the disk sub-header.
    pub start_date: i64,
    /// Seconds since session midnight recorded as the session start time.
    pub start_time: f64,
    /// Ordered variable headers describing each frame.
    pub variables: Vec<Variable>,
}

/// Constructs the common scalar form of a profile variable.
fn variable(
    name: &'static str,
    data_type: VariableType,
    offset: i32,
    units: &'static str,
    description: &'static str,
) -> Variable {
    Variable {
        name,
        data_type,
        offset,
        count: 1,
        units,
        description,
        count_as_time: false,
    }
}

/// Returns the eight variables shared by every generated profile.
fn base_variables() -> Vec<Variable> {
    use VariableType::{Double, Float, Integer};
    vec![
        variable("SessionTime", Double, 0, "s", "Seconds since session start"),
        variable("Speed", Float, 8, "m/s", "Vehicle speed"),
        variable("LapDist", Float, 12, "m", "Distance around lap"),
        variable("LapCompleted", Integer, 16, "", "Completed laps"),
        variable("Brake", Float, 20, "%", "Brake pedal input"),
        variable("Throttle", Float, 24, "%", "Throttle pedal input"),
        variable("RPM", Float, 28, "revs/min", "Engine speed"),
        variable("Gear", Integer, 32, "", "Selected gear"),
    ]
}

/// Returns the canonical small, medium, and large profiles in manifest order.
pub(crate) fn profiles() -> Vec<Profile> {
    use VariableType::{BitField, Boolean, Float};
    let small = Profile {
        name: "profile_small",
        seed: 10_001,
        track_name: "generated small",
        track_display_name: "Generated Small Circuit",
        session_name: "Practice",
        tick_rate: 60,
        frame_count: 12,
        frame_size: 48,
        lap_count: 1,
        start_date: 1_775_785_000,
        start_time: 120.0,
        variables: base_variables(),
    };
    let mut medium_variables = base_variables();
    medium_variables.extend([
        variable(
            "SteeringWheelAngle",
            Float,
            36,
            "rad",
            "Steering wheel angle",
        ),
        variable("FuelLevel", Float, 40, "l", "Fuel level"),
    ]);
    let medium = Profile {
        name: "profile_medium",
        seed: 20_002,
        track_name: "generated medium",
        track_display_name: "Generated Medium Circuit",
        session_name: "Qualify",
        tick_rate: 60,
        frame_count: 24,
        frame_size: 64,
        lap_count: 3,
        start_date: 1_775_786_000,
        start_time: 240.0,
        variables: medium_variables,
    };
    let mut large_variables = base_variables();
    large_variables.extend([
        variable(
            "SteeringWheelAngle",
            Float,
            36,
            "rad",
            "Steering wheel angle",
        ),
        variable("FuelLevel", Float, 40, "l", "Fuel level"),
        variable("TrackTemp", Float, 44, "C", "Track temperature"),
        variable("OnPitRoad", Boolean, 48, "", "Whether car is on pit road"),
        variable("SessionFlags", BitField, 52, "", "Session status flags"),
    ]);
    let large = Profile {
        name: "profile_large",
        seed: 30_003,
        track_name: "generated large",
        track_display_name: "Generated Large Circuit",
        session_name: "Race",
        tick_rate: 60,
        frame_count: 48,
        frame_size: 96,
        lap_count: 8,
        start_date: 1_775_787_000,
        start_time: 360.0,
        variables: large_variables,
    };
    vec![small, medium, large]
}

/// Root object serialized to `test-data/ibt/manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FixtureManifest {
    /// Manifest contract version. Only version 1 is supported.
    pub schema_version: u32,
    /// Human-readable identifier for the generating tool.
    pub generated_by: String,
    /// Shared binary-layout values for all listed fixtures.
    pub layout: FixtureManifestLayout,
    /// Authoritative ordered list of deterministic fixtures.
    pub fixtures: Vec<IbtFixture>,
}

/// Shared SDK wire sizes and offset rule recorded in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FixtureManifestLayout {
    /// Legacy name for the 112-byte main SDK header size.
    pub live_header_prefix_size: usize,
    /// Composite IBT preamble size: main header plus disk sub-header.
    pub ibt_header_size: usize,
    /// Size of the IBT-only disk sub-header.
    pub disk_sub_header_size: usize,
    /// Size of one SDK variable header.
    pub variable_header_size: usize,
    /// Descriptive invariant locating the disk sub-header.
    pub disk_sub_header_offset_rule: String,
}

/// Manifest metadata and expected invariants for one generated IBT file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IbtFixture {
    /// Stable semantic profile name.
    pub name: String,
    /// Repository-relative IBT file path.
    pub path: String,
    /// Repository-relative companion session-YAML path.
    pub session_yaml_path: String,
    /// Deterministic RNG seed used by the profile.
    pub seed: u64,
    /// Expected header tick rate.
    pub tick_rate: i32,
    /// Expected number of variable headers.
    pub num_vars: i32,
    /// Expected size of each telemetry frame.
    pub frame_size: usize,
    /// Expected number of telemetry frames.
    pub num_frames: usize,
    /// Offset of the first variable header; currently 144.
    pub var_header_offset: i32,
    /// Offset of the disk sub-header; currently 112.
    pub disk_sub_header_offset: i32,
    /// Expected session-info update counter.
    pub session_info_update: i32,
    /// Byte length of the embedded session YAML.
    pub session_info_len: i32,
    /// Byte offset of the embedded session YAML.
    pub session_info_offset: i32,
    /// Number of buffers advertised by the IBT header.
    pub num_buf: i32,
    /// Expected disk sub-header values.
    pub disk_header: IbtDiskHeaderManifest,
    /// Lowercase SHA-256 digest of the complete IBT file.
    pub sha256: String,
    /// Variables the verifier requires the generated schema to expose.
    pub required_variables: Vec<IbtVariableManifest>,
}

/// Manifest representation of IBT-only disk metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IbtDiskHeaderManifest {
    /// Expected Unix session-start timestamp.
    pub start_date: i64,
    /// Expected seconds-since-midnight session start.
    pub start_time: f64,
    /// Expected seconds-since-midnight session end.
    pub end_time: f64,
    /// Expected completed-lap count.
    pub lap_count: i32,
    /// Expected number of telemetry records.
    pub record_count: i32,
}

/// Manifest representation of required variable metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IbtVariableManifest {
    /// Expected variable name.
    pub name: String,
    /// Historical manifest spelling of the SDK storage type.
    pub data_type: String,
    /// Expected byte offset within a frame.
    pub offset: usize,
    /// Expected scalar or array element count.
    pub count: usize,
    /// Expected unit string.
    pub units: String,
}
