//! iRacing primitive schema generator.
//!
//! Emits a JSON Schema (YAML-serialized) describing the exported `irsdk_*` primitive wrappers from
//! `iracing_sdk::types` (enum and bitflag families).

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
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

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args { output_path } = Args::parse();

    let schema = schema_for!(IrsdkPrimitivesSchema);
    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(
        path = %output_path.display(),
        "Wrote iRacing primitive schema"
    );

    Ok(())
}
