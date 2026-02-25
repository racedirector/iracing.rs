//! iRacing primitive schema generator.
//!
//! Emits a JSON Schema (YAML-serialized) describing the exported `irsdk_*` primitive wrappers from
//! `iracing_sdk::types` (enum and bitflag families).

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::Parser;
use schemars::{JsonSchema, Schema, schema_for};
use serde::Serialize;
use serde_json::{Map, Value};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the generated schema YAML should be written.
    #[arg(short, long, default_value = "iracing-primitives-schema.yml")]
    output_path: PathBuf,
}

#[derive(Debug, Serialize, JsonSchema)]
struct IrsdkPrimitivesSchema {
    #[serde(rename = "irsdk_StatusField")]
    status_field: iracing_sdk::StatusField,
    #[serde(rename = "irsdk_TrkLoc")]
    trk_loc: iracing_sdk::TrackLocation,
    #[serde(rename = "irsdk_TrkSurf")]
    trk_surf: iracing_sdk::TrackSurface,
    #[serde(rename = "irsdk_SessionState")]
    session_state: iracing_sdk::SessionState,
    #[serde(rename = "irsdk_CarLeftRight")]
    car_left_right: iracing_sdk::CarLeftRight,
    #[serde(rename = "irsdk_PitSvStatus")]
    pit_sv_status: iracing_sdk::PitServiceStatus,
    #[serde(rename = "irsdk_PaceMode")]
    pace_mode: iracing_sdk::PaceMode,
    #[serde(rename = "irsdk_TrackWetness")]
    track_wetness: iracing_sdk::TrackWetness,
    #[serde(rename = "irsdk_BroadcastMsg")]
    broadcast_msg: iracing_sdk::BroadcastMessage,
    #[serde(rename = "irsdk_ChatCommandMode")]
    chat_command_mode: iracing_sdk::ChatCommandMode,
    #[serde(rename = "irsdk_PitCommandMode")]
    pit_command_mode: iracing_sdk::PitCommandMode,
    #[serde(rename = "irsdk_TelemetryCommandMode")]
    telemetry_command_mode: iracing_sdk::TelemetryCommandMode,
    #[serde(rename = "irsdk_RpyStateMode")]
    rpy_state_mode: iracing_sdk::ReplayStateMode,
    #[serde(rename = "irsdk_ReloadTexturesMode")]
    reload_textures_mode: iracing_sdk::ReloadTexturesMode,
    #[serde(rename = "irsdk_RpySrchMode")]
    rpy_srch_mode: iracing_sdk::ReplaySearchMode,
    #[serde(rename = "irsdk_RpyPosMode")]
    rpy_pos_mode: iracing_sdk::ReplayPositionMode,
    #[serde(rename = "irsdk_FFBCommandMode")]
    ffb_command_mode: iracing_sdk::FfbCommandMode,
    #[serde(rename = "irsdk_csMode")]
    cs_mode: iracing_sdk::CameraSwitchFocus,
    #[serde(rename = "irsdk_VideoCaptureMode")]
    video_capture_mode: iracing_sdk::VideoCaptureMode,
    #[serde(rename = "irsdk_EngineWarnings")]
    engine_warnings: iracing_sdk::EngineWarnings,
    #[serde(rename = "irsdk_Flags")]
    flags: iracing_sdk::SessionFlags,
    #[serde(rename = "irsdk_CameraState")]
    camera_state: iracing_sdk::CameraState,
    #[serde(rename = "irsdk_PitSvFlags")]
    pit_sv_flags: iracing_sdk::PitServiceFlags,
    #[serde(rename = "irsdk_PaceFlags")]
    pace_flags: iracing_sdk::PaceFlags,
    #[serde(rename = "irsdk_IncidentFlags")]
    incident_flags: iracing_sdk::IncidentFlags,
}

fn schema_def_object_mut<'a>(
    schema: &'a mut Schema,
    def_name: &str,
) -> Result<&'a mut Map<String, Value>> {
    schema
        .ensure_object()
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .and_then(|defs| defs.get_mut(def_name))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("missing $defs.{def_name} while enriching primitive schema"))
}

fn named_value_entries(values: &[(&'static str, i64)]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|(name, value)| {
                let mut entry = Map::new();
                entry.insert("name".into(), (*name).into());
                entry.insert("value".into(), (*value).into());
                Value::Object(entry)
            })
            .collect(),
    )
}

fn annotate_named_values(
    schema: &mut Schema,
    def_name: &str,
    kind: &str,
    values: &[(&'static str, i64)],
    known_mask: Option<u32>,
) -> Result<()> {
    let def = schema_def_object_mut(schema, def_name)?;
    def.insert("x-irsdk-kind".into(), kind.into());
    def.insert("x-irsdk-values".into(), named_value_entries(values));
    if let Some(mask) = known_mask {
        def.insert("x-irsdk-known-mask".into(), (mask as u64).into());
    }
    Ok(())
}

fn annotate_incident_values(schema: &mut Schema) -> Result<()> {
    let def = schema_def_object_mut(schema, "IncidentFlags")?;
    def.insert("x-irsdk-kind".into(), "incident-flags".into());

    let mut masks = Map::new();
    masks.insert(
        "report".into(),
        (iracing_sdk::IncidentFlags::REP_MASK as u64).into(),
    );
    masks.insert(
        "penalty".into(),
        (iracing_sdk::IncidentFlags::PEN_MASK as u64).into(),
    );
    def.insert("x-irsdk-masks".into(), Value::Object(masks));
    def.insert(
        "x-irsdk-report-codes".into(),
        named_value_entries(iracing_sdk::IncidentFlags::SCHEMA_REPORT_CODES),
    );
    def.insert(
        "x-irsdk-penalty-codes".into(),
        named_value_entries(iracing_sdk::IncidentFlags::SCHEMA_PENALTY_CODES),
    );
    Ok(())
}

fn annotate_primitive_values(schema: &mut Schema) -> Result<()> {
    annotate_named_values(
        schema,
        "StatusField",
        "enum",
        iracing_sdk::StatusField::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "TrackLocation",
        "enum",
        iracing_sdk::TrackLocation::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "TrackSurface",
        "enum",
        iracing_sdk::TrackSurface::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "SessionState",
        "enum",
        iracing_sdk::SessionState::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "CarLeftRight",
        "enum",
        iracing_sdk::CarLeftRight::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "PitServiceStatus",
        "enum",
        iracing_sdk::PitServiceStatus::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "PaceMode",
        "enum",
        iracing_sdk::PaceMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "TrackWetness",
        "enum",
        iracing_sdk::TrackWetness::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "BroadcastMessage",
        "enum",
        iracing_sdk::BroadcastMessage::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "ChatCommandMode",
        "enum",
        iracing_sdk::ChatCommandMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "PitCommandMode",
        "enum",
        iracing_sdk::PitCommandMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "TelemetryCommandMode",
        "enum",
        iracing_sdk::TelemetryCommandMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "ReplayStateMode",
        "enum",
        iracing_sdk::ReplayStateMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "ReloadTexturesMode",
        "enum",
        iracing_sdk::ReloadTexturesMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "ReplaySearchMode",
        "enum",
        iracing_sdk::ReplaySearchMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "ReplayPositionMode",
        "enum",
        iracing_sdk::ReplayPositionMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "FfbCommandMode",
        "enum",
        iracing_sdk::FfbCommandMode::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "CameraSwitchFocus",
        "enum",
        iracing_sdk::CameraSwitchFocus::SCHEMA_VALUES,
        None,
    )?;
    annotate_named_values(
        schema,
        "VideoCaptureMode",
        "enum",
        iracing_sdk::VideoCaptureMode::SCHEMA_VALUES,
        None,
    )?;

    annotate_named_values(
        schema,
        "EngineWarnings",
        "bitflags",
        iracing_sdk::EngineWarnings::SCHEMA_VALUES,
        Some(iracing_sdk::EngineWarnings::SCHEMA_KNOWN_MASK),
    )?;
    annotate_named_values(
        schema,
        "SessionFlags",
        "bitflags",
        iracing_sdk::SessionFlags::SCHEMA_VALUES,
        Some(iracing_sdk::SessionFlags::SCHEMA_KNOWN_MASK),
    )?;
    annotate_named_values(
        schema,
        "CameraState",
        "bitflags",
        iracing_sdk::CameraState::SCHEMA_VALUES,
        Some(iracing_sdk::CameraState::SCHEMA_KNOWN_MASK),
    )?;
    annotate_named_values(
        schema,
        "PitServiceFlags",
        "bitflags",
        iracing_sdk::PitServiceFlags::SCHEMA_VALUES,
        Some(iracing_sdk::PitServiceFlags::SCHEMA_KNOWN_MASK),
    )?;
    annotate_named_values(
        schema,
        "PaceFlags",
        "bitflags",
        iracing_sdk::PaceFlags::SCHEMA_VALUES,
        Some(iracing_sdk::PaceFlags::SCHEMA_KNOWN_MASK),
    )?;

    annotate_incident_values(schema)?;

    Ok(())
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args { output_path } = Args::parse();

    let mut schema = schema_for!(IrsdkPrimitivesSchema);
    annotate_primitive_values(&mut schema)?;

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(
        path = %output_path.display(),
        "Wrote iRacing primitive schema"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn enriched_schema() -> Schema {
        let mut schema = schema_for!(IrsdkPrimitivesSchema);
        annotate_primitive_values(&mut schema).expect("schema enrichment should succeed");
        schema
    }

    fn def_object<'a>(schema: &'a Schema, name: &str) -> &'a Map<String, Value> {
        schema
            .as_value()
            .get("$defs")
            .and_then(Value::as_object)
            .and_then(|defs| defs.get(name))
            .and_then(Value::as_object)
            .expect("definition should exist")
    }

    fn to_value_map(entries: &Value) -> BTreeMap<String, i64> {
        entries
            .as_array()
            .expect("entries should be an array")
            .iter()
            .map(|entry| {
                let obj = entry.as_object().expect("entry should be an object");
                let name = obj
                    .get("name")
                    .and_then(Value::as_str)
                    .expect("entry.name should be a string")
                    .to_string();
                let value = obj
                    .get("value")
                    .and_then(Value::as_i64)
                    .expect("entry.value should be an integer");
                (name, value)
            })
            .collect()
    }

    #[test]
    fn enriches_bitflag_defs_with_known_values_and_mask() {
        let schema = enriched_schema();
        let session_flags = def_object(&schema, "SessionFlags");

        assert_eq!(
            session_flags.get("x-irsdk-kind"),
            Some(&Value::String("bitflags".to_string()))
        );

        let values = to_value_map(
            session_flags
                .get("x-irsdk-values")
                .expect("values metadata should exist"),
        );
        assert_eq!(
            values.get("CHECKERED"),
            Some(&(iracing_sdk::irsdk_flags::flags::CHECKERED as i64))
        );
        assert_eq!(
            values.get("START_GO"),
            Some(&(iracing_sdk::irsdk_flags::flags::START_GO as i64))
        );
        assert_eq!(
            session_flags
                .get("x-irsdk-known-mask")
                .and_then(Value::as_u64),
            Some(iracing_sdk::SessionFlags::SCHEMA_KNOWN_MASK as u64)
        );
    }

    #[test]
    fn enriches_enum_defs_with_raw_numeric_values() {
        let schema = enriched_schema();
        let trk_loc = def_object(&schema, "TrackLocation");

        assert_eq!(
            trk_loc.get("x-irsdk-kind"),
            Some(&Value::String("enum".to_string()))
        );

        let values = to_value_map(
            trk_loc
                .get("x-irsdk-values")
                .expect("values metadata should exist"),
        );
        assert_eq!(
            values.get("NotInWorld"),
            Some(&(iracing_sdk::irsdk_flags::trk_loc::NOT_IN_WORLD as i64))
        );
        assert_eq!(
            values.get("OnTrack"),
            Some(&(iracing_sdk::irsdk_flags::trk_loc::ON_TRACK as i64))
        );
    }

    #[test]
    fn enriches_incident_flags_with_masks_and_code_tables() {
        let schema = enriched_schema();
        let incident = def_object(&schema, "IncidentFlags");

        assert_eq!(
            incident.get("x-irsdk-kind"),
            Some(&Value::String("incident-flags".to_string()))
        );
        assert_eq!(
            incident
                .get("x-irsdk-masks")
                .and_then(Value::as_object)
                .and_then(|masks| masks.get("report"))
                .and_then(Value::as_u64),
            Some(iracing_sdk::IncidentFlags::REP_MASK as u64)
        );
        assert_eq!(
            incident
                .get("x-irsdk-masks")
                .and_then(Value::as_object)
                .and_then(|masks| masks.get("penalty"))
                .and_then(Value::as_u64),
            Some(iracing_sdk::IncidentFlags::PEN_MASK as u64)
        );

        let report_codes = to_value_map(
            incident
                .get("x-irsdk-report-codes")
                .expect("report code metadata should exist"),
        );
        assert_eq!(
            report_codes.get("REP_COLLISION_WITH_CAR"),
            Some(&(iracing_sdk::irsdk_flags::incident::REP_COLLISION_WITH_CAR as i64))
        );

        let penalty_codes = to_value_map(
            incident
                .get("x-irsdk-penalty-codes")
                .expect("penalty code metadata should exist"),
        );
        assert_eq!(
            penalty_codes.get("PEN_4X"),
            Some(&(iracing_sdk::irsdk_flags::incident::PEN_4X as i64))
        );
    }
}
