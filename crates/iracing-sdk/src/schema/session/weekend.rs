//! Weekend and track information
//!
//! This module contains weekend-related information from iRacing session data,
//! including track details, weather conditions, and session configuration.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

/// Weekend and track information from iRacing
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct WeekendInfo {
    /// Track name
    pub track_name: String,
    /// Track ID
    #[serde(rename = "TrackID")]
    pub track_id: Option<i32>,
    /// Track length
    pub track_length: String,
    /// Official track length
    pub track_length_official: Option<String>,
    /// Track display name
    pub track_display_name: String,
    /// Track short name
    pub track_display_short_name: Option<String>,
    /// Track configuration name
    pub track_config_name: Option<String>,
    /// Track city
    pub track_city: Option<String>,
    /// Track state/province
    pub track_state: Option<String>,
    /// Track country
    pub track_country: Option<String>,
    /// Track altitude
    pub track_altitude: Option<String>,
    /// Track latitude in meters
    pub track_latitude: Option<String>,
    /// Track longitude in meters
    pub track_longitude: Option<String>,
    /// Track north offset in radians
    pub track_north_offset: Option<String>,
    /// Track number of turns
    pub track_num_turns: Option<i32>,
    /// Track pit speed limit
    pub track_pit_speed_limit: Option<String>,
    /// Track pace speed
    pub track_pace_speed: Option<String>,
    /// Track number of pit stalls
    pub track_num_pit_stalls: Option<i32>,
    /// Track type (road course, oval, etc.)
    pub track_type: Option<String>,
    /// Track direction (neutral, clockwise, counter-clockwise)
    pub track_direction: Option<String>,
    /// Track weather type (Static, Dynamic)
    pub track_weather_type: Option<String>,
    /// Track skies condition
    pub track_skies: Option<String>,
    /// Track surface temperature
    pub track_surface_temp: Option<String>,
    /// Track surface temperature (crew-facing)
    pub track_surface_temp_crew: Option<String>,
    /// Track air temperature
    pub track_air_temp: Option<String>,
    /// Track air pressure
    pub track_air_pressure: Option<String>,
    /// Track air density
    pub track_air_density: Option<String>,
    /// Track wind velocity
    pub track_wind_vel: Option<String>,
    /// Track wind direction
    pub track_wind_dir: Option<String>,
    /// Track relative humidity
    pub track_relative_humidity: Option<String>,
    /// Track fog level percentage
    pub track_fog_level: Option<String>,
    /// Track precipitation percentage
    pub track_precipitation: Option<String>,
    /// Track cleanup level
    pub track_cleanup: Option<i32>,
    /// Track dynamic track enabled
    pub track_dynamic_track: Option<i32>,
    /// Track version
    pub track_version: Option<String>,
    /// Series ID
    #[serde(rename = "SeriesID")]
    pub series_id: Option<i32>,
    /// Season ID
    #[serde(rename = "SeasonID")]
    pub season_id: Option<i32>,
    /// Session ID
    #[serde(rename = "SessionID")]
    pub session_id: Option<i32>,
    /// Sub-session ID (for splits)
    #[serde(rename = "SubSessionID")]
    pub sub_session_id: Option<i32>,
    /// League ID
    #[serde(rename = "LeagueID")]
    pub league_id: Option<i32>,
    /// Official session flag
    pub official: Option<i32>,
    /// Race week number
    pub race_week: Option<i32>,
    /// Event type
    pub event_type: Option<String>,
    /// Category (Road, Oval, etc.)
    pub category: Option<String>,
    /// Simulation mode (full, fixed, open)
    pub sim_mode: Option<String>,
    /// Team racing enabled
    pub team_racing: Option<i32>,
    /// Minimum number of drivers
    pub min_drivers: Option<i32>,
    /// Maximum number of drivers
    pub max_drivers: Option<i32>,
    /// Drive through/stop-go rule set
    #[serde(rename = "DCRuleSet")]
    pub dc_rule_set: Option<String>,
    /// Qualifier must start race flag
    pub qualifier_must_start_race: Option<i32>,
    /// Number of car classes
    pub num_car_classes: Option<i32>,
    /// Number of car types
    pub num_car_types: Option<i32>,
    /// Heat racing enabled
    pub heat_racing: Option<i32>,
    /// Build type (Release, Beta, etc.)
    pub build_type: Option<String>,
    /// Build target (Members, AI, etc.)
    pub build_target: Option<String>,
    /// Build version
    pub build_version: Option<String>,
    /// Race farm identifier
    pub race_farm: Option<String>,
    /// Telemetry options
    pub telemetry_options: Option<TelemetryOptions>,
    /// Weekend options
    pub weekend_options: Option<WeekendOptions>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Telemetry recording options
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct TelemetryOptions {
    /// Telemetry disk file path
    pub telemetry_disk_file: Option<String>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Weekend session options and configuration
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct WeekendOptions {
    /// Number of starters
    pub num_starters: Option<i32>,
    /// Starting grid format
    pub starting_grid: Option<String>,
    /// Qualifying scoring method
    pub qualify_scoring: Option<String>,
    /// Course cautions setting
    pub course_cautions: Option<String>,
    /// Standing start enabled
    pub standing_start: Option<i32>,
    /// Short parade lap enabled
    pub short_parade_lap: Option<i32>,
    /// Restart format
    pub restarts: Option<String>,
    /// Weather type
    pub weather_type: Option<String>,
    /// Sky condition
    pub skies: Option<String>,
    /// Wind direction
    pub wind_direction: Option<String>,
    /// Wind speed
    pub wind_speed: Option<String>,
    /// Weather temperature
    pub weather_temp: Option<String>,
    /// Relative humidity
    pub relative_humidity: Option<String>,
    /// Fog level
    pub fog_level: Option<String>,
    /// Time of day
    pub time_of_day: Option<String>,
    /// Session date
    pub date: Option<String>,
    /// Earth rotation speedup factor
    pub earth_rotation_speedup_factor: Option<i32>,
    /// Unofficial session flag
    pub unofficial: Option<i32>,
    /// Commercial mode
    pub commercial_mode: Option<String>,
    /// Night mode setting
    pub night_mode: Option<String>,
    /// Fixed setup required
    pub is_fixed_setup: Option<i32>,
    /// Strict laps checking
    pub strict_laps_checking: Option<String>,
    /// Open registration flag
    pub has_open_registration: Option<i32>,
    /// Hardcore level
    pub hardcore_level: Option<i32>,
    /// Number of joker laps
    pub num_joker_laps: Option<i32>,
    /// Incident limit
    pub incident_limit: Option<String>,
    /// Fast repairs limit
    pub fast_repairs_limit: Option<String>,
    /// Green-white-checkered limit
    pub green_white_checkered_limit: Option<i32>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}
