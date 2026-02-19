//! Driver information structures
//!
//! This module contains driver-related data structures including driver lists,
//! driver details, and tire compound information.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema-discovery")]
use std::collections::HashMap;

/// Driver information data containing current driver info + drivers list
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct DriverInfoData {
    /// Current driver car index
    pub driver_car_idx: Option<i32>,
    /// Current driver user ID
    #[serde(rename = "DriverUserID")]
    pub driver_user_id: Option<i32>,
    /// Pace car index
    pub pace_car_idx: Option<i32>,
    /// Whether driver is admin
    pub driver_is_admin: Option<i32>,
    /// Driver head position X
    pub driver_head_pos_x: Option<f64>,
    /// Driver head position Y
    pub driver_head_pos_y: Option<f64>,
    /// Driver head position Z
    pub driver_head_pos_z: Option<f64>,
    /// Whether driver car is electric
    pub driver_car_is_electric: Option<i32>,
    /// Driver car idle RPM
    #[serde(rename = "DriverCarIdleRPM")]
    pub driver_car_idle_rpm: Option<f64>,
    /// Driver car redline RPM
    pub driver_car_red_line: Option<f64>,
    /// Driver car engine cylinder count
    pub driver_car_eng_cylinder_count: Option<i32>,
    /// Driver car fuel density (kg per liter)
    pub driver_car_fuel_kg_per_ltr: Option<f64>,
    /// Driver car fuel tank capacity (liters)
    pub driver_car_fuel_max_ltr: Option<f64>,
    /// Driver car maximum fuel percentage
    pub driver_car_max_fuel_pct: Option<f64>,
    /// Driver car number of forward gears
    pub driver_car_gear_num_forward: Option<i32>,
    /// Driver car neutral gear present
    pub driver_car_gear_neutral: Option<i32>,
    /// Driver car reverse gear present
    pub driver_car_gear_reverse: Option<i32>,
    /// Driver gearbox type
    pub driver_gearbox_type: Option<String>,
    /// Driver gearbox control type
    pub driver_gearbox_control_type: Option<String>,
    /// Driver car shift aid
    pub driver_car_shift_aid: Option<String>,
    /// Driver car shift light first RPM
    #[serde(rename = "DriverCarSLFirstRPM")]
    pub driver_car_sl_first_rpm: Option<f64>,
    /// Driver car shift light shift RPM
    #[serde(rename = "DriverCarSLShiftRPM")]
    pub driver_car_sl_shift_rpm: Option<f64>,
    /// Driver car shift light last RPM
    #[serde(rename = "DriverCarSLLastRPM")]
    pub driver_car_sl_last_rpm: Option<f64>,
    /// Driver car shift light blink RPM
    #[serde(rename = "DriverCarSLBlinkRPM")]
    pub driver_car_sl_blink_rpm: Option<f64>,
    /// Driver car version
    pub driver_car_version: Option<String>,
    /// Driver pit entrance track percentage
    pub driver_pit_trk_pct: Option<f64>,
    /// Driver car estimated lap time
    pub driver_car_est_lap_time: Option<f64>,
    /// Driver setup name
    pub driver_setup_name: Option<String>,
    /// Driver setup modified flag
    pub driver_setup_is_modified: Option<i32>,
    /// Driver setup load type name
    pub driver_setup_load_type_name: Option<String>,
    /// Driver setup passed tech inspection
    pub driver_setup_passed_tech: Option<i32>,
    /// Driver incident count
    pub driver_incident_count: Option<i32>,
    /// Driver brake curving factor
    pub driver_brake_curving_factor: Option<f64>,
    /// Available tire compounds
    pub driver_tires: Option<Vec<DriverTire>>,
    /// List of all drivers in session
    pub drivers: Option<Vec<Driver>>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(feature = "codegen", schemars(with = "std::collections::HashMap<String, serde_json::Value>"))]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Driver tire compound information
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct DriverTire {
    /// Tire index
    pub tire_index: Option<i32>,
    /// Tire compound type
    pub tire_compound_type: Option<String>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(feature = "codegen", schemars(with = "std::collections::HashMap<String, serde_json::Value>"))]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}

/// Individual driver data (from Drivers list)
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct Driver {
    /// Car index number
    pub car_idx: i32,
    /// Driver name
    pub user_name: String,
    /// Driver abbreviation
    pub abbrev_name: Option<String>,
    /// Driver initials
    pub initials: Option<String>,
    /// User ID
    #[serde(rename = "UserID")]
    pub user_id: Option<i32>,
    /// Team ID
    #[serde(rename = "TeamID")]
    pub team_id: Option<i32>,
    /// Team name
    pub team_name: Option<String>,
    /// Car number (display)
    pub car_number: Option<String>,
    /// Car number raw (numeric with class prefix)
    pub car_number_raw: Option<i32>,
    /// Car path (directory name)
    pub car_path: Option<String>,
    /// Car class ID
    #[serde(rename = "CarClassID")]
    pub car_class_id: Option<i32>,
    /// Car ID
    #[serde(rename = "CarID")]
    pub car_id: Option<i32>,
    /// Car screen name
    pub car_screen_name: Option<String>,
    /// Car screen name short
    pub car_screen_name_short: Option<String>,
    /// Car configuration ID
    pub car_cfg: Option<i32>,
    /// Car configuration name
    pub car_cfg_name: Option<String>,
    /// Car custom paint extension
    pub car_cfg_custom_paint_ext: Option<String>,
    /// Car class short name
    pub car_class_short_name: Option<String>,
    /// Car class relative speed
    pub car_class_rel_speed: Option<i32>,
    /// Car class license level requirement
    pub car_class_license_level: Option<i32>,
    /// Car class maximum fuel percentage
    pub car_class_max_fuel_pct: Option<String>,
    /// Car class weight penalty
    pub car_class_weight_penalty: Option<String>,
    /// Car class power adjustment (BOP)
    pub car_class_power_adjust: Option<String>,
    /// Car class dry tire set limit
    pub car_class_dry_tire_set_limit: Option<String>,
    /// Car class color (hex)
    pub car_class_color: Option<String>,
    /// Car class estimated lap time
    pub car_class_est_lap_time: Option<f64>,
    /// Whether this is a pace car
    pub car_is_pace_car: Option<i32>,
    /// Whether this is AI
    #[serde(rename = "CarIsAI")]
    pub car_is_ai: Option<i32>,
    /// Whether this is an electric car
    pub car_is_electric: Option<i32>,
    /// iRating
    pub i_rating: Option<i32>,
    /// License level
    pub lic_level: Option<i32>,
    /// License sub-level
    pub lic_sub_level: Option<i32>,
    /// License string (display)
    pub lic_string: Option<String>,
    /// License color (hex)
    pub lic_color: Option<String>,
    /// Club ID
    #[serde(rename = "ClubID")]
    pub club_id: Option<i32>,
    /// Club name
    pub club_name: Option<String>,
    /// Division ID
    #[serde(rename = "DivisionID")]
    pub division_id: Option<i32>,
    /// Division name
    pub division_name: Option<String>,
    /// Whether this is a spectator
    pub is_spectator: Option<i32>,
    /// Car design string (livery colors)
    ///
    /// **Note**: Known to contain malformed data in AI races. Parse failures
    /// will result in None. Application should handle missing design strings gracefully.
    pub car_design_str: Option<String>,
    /// Helmet design string
    ///
    /// **Note**: Known to contain malformed data in AI races. Parse failures
    /// will result in None. Application should handle missing design strings gracefully.
    pub helmet_design_str: Option<String>,
    /// Suit design string
    ///
    /// **Note**: Known to contain malformed data in AI races. Parse failures
    /// will result in None. Application should handle missing design strings gracefully.
    pub suit_design_str: Option<String>,
    /// Body type (avatar)
    pub body_type: Option<i32>,
    /// Face type (avatar)
    pub face_type: Option<i32>,
    /// Helmet type
    pub helmet_type: Option<i32>,
    /// Flair ID
    #[serde(rename = "FlairID")]
    pub flair_id: Option<i32>,
    /// Flair name
    pub flair_name: Option<String>,
    /// Car number design string
    ///
    /// **Note**: Known to contain malformed data in AI races. Parse failures
    /// will result in None. Application should handle missing design strings gracefully.
    pub car_number_design_str: Option<String>,
    /// Car sponsor 1
    #[serde(rename = "CarSponsor_1")]
    pub car_sponsor_1: Option<i32>,
    /// Car sponsor 2
    #[serde(rename = "CarSponsor_2")]
    pub car_sponsor_2: Option<i32>,
    /// Current driver incident count
    pub cur_driver_incident_count: Option<i32>,
    /// Team incident count
    pub team_incident_count: Option<i32>,
    /// Unknown fields discovered during parsing (requires schema-discovery feature)
    #[cfg(feature = "schema-discovery")]
    #[serde(flatten)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[cfg_attr(feature = "codegen", schemars(with = "std::collections::HashMap<String, serde_json::Value>"))]
    pub unknown_fields: HashMap<String, serde_yaml_ng::Value>,
}
