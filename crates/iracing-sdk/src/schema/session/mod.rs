//! Typed session YAML deserialization and optional unknown-field discovery.
//!
//! [`SessionInfo::parse`] accepts already decoded text and deserializes it with
//! serde. It performs no source acquisition, byte decoding, control-character
//! cleanup, YAML quoting repair, version caching, or publication. Each call
//! creates a new typed value; source and runtime policies decide when to call it.
//!
//! The byte conversion into [`SessionInfo`] composes separate owners:
//! [`crate::SessionInfoBytes`] retains the advertised region;
//! [`crate::SessionInfoPayload`] bounds the NUL-terminated text and decodes its
//! declared encoding; the internal `IRacingSessionString` removes selected
//! control characters and rejects empty cleaned text before deserialization.
//! That cleanup does not rewrite quotes, escape paths, or repair arbitrary YAML.
//!
//! Fields and serde attributes on the domain types own required/defaulted
//! sections and PascalCase names. With `schema-discovery`, participating types
//! retain unmodeled keys in flattened maps; the discovery module owns reporting helpers.
//! Without that feature, unmodeled keys are not retained in those maps.
//! Generated schemas describe model output; captured examples are evidence,
//! not a second parser specification. See the
//! [reference provenance guide](https://github.com/racedirector/iracing.rs/blob/main/docs/reference/README.md).
//!
//! [Parsing tests](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk/src/schema/session/tests.rs)
//! exercise typed fields, required sections, byte-conversion order, error
//! contexts, and feature-gated unknown fields. Byte decoding and sanitization
//! have their own tests beside those implementations. Performance observations
//! require revision/workload provenance; this module makes no timing guarantee.

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

use crate::{IRacingSDKError, IRacingSessionString, Result, SessionInfoBytes};

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
    /// Deserialize decoded YAML into a fresh typed session value.
    ///
    /// The caller supplies valid YAML text compatible with the serde model. This
    /// method does not decode bytes, remove control characters, repair YAML, or
    /// cache a previous result. Convert a [`SessionInfoBytes`] snapshot with
    /// [`Self::try_from`] to use the SDK's decoding and sanitization path.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Parse`] with the context
    /// `SessionInfo deserialization` when YAML syntax or typed model
    /// deserialization fails. Required and defaulted fields are defined on the
    /// domain types; unknown-field retention depends on `schema-discovery`.
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

impl TryFrom<SessionInfoBytes> for SessionInfo {
    type Error = IRacingSDKError;

    /// Decode the bounded payload, sanitize its text, then deserialize once.
    ///
    /// Consumes the owned snapshot without acquiring a source or caching the
    /// parsed value. Propagates empty-cleaned-text and deserialization errors;
    /// the decoding boundary itself replaces or falls back for malformed text
    /// according to [`crate::SessionInfoPayload::decode`].
    fn try_from(value: SessionInfoBytes) -> Result<Self, Self::Error> {
        let session_info = IRacingSessionString::try_from(value)?;
        SessionInfo::try_from(session_info)
    }
}

impl TryFrom<IRacingSessionString> for SessionInfo {
    type Error = IRacingSDKError;

    /// Deserialize sanitized text without repeating byte decoding or cleanup.
    fn try_from(value: IRacingSessionString) -> Result<Self> {
        let session_info = String::from(value);
        SessionInfo::parse(&session_info)
    }
}

#[cfg(test)]
mod tests;
