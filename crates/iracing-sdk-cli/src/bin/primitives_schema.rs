//! iRacing primitive schema generator.
//!
//! Emits a JSON Schema (YAML-serialized) describing every exported `irsdk_*` primitive wrapper
//! from `iracing_sdk::types` — enumerations (e.g. `irsdk_SessionState`) and bitflag families
//! (e.g. `irsdk_Flags`). The output is suitable as a shared `$defs` bank referenced by the
//! variable schema when `--annotate` is passed to `disk_variable_schema` or
//! `live_variable_schema`.
//!
//! # Usage
//! ```text
//! iracing_primitives_schema [--output-path <SCHEMA.yml>]
//! ```

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use iracing_sdk::irsdk::*;
use serde::Serialize;

/// # iRacing Enums and BitFlags Schema
/// A JSON schema reference of all available flags and enums in the iRacing SDK.
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct IrsdkPrimitivesSchema {
    #[serde(rename = "irsdk_StatusField")]
    status_field: StatusField,
    #[serde(rename = "irsdk_TrkLoc")]
    trk_loc: TrackLocation,
    #[serde(rename = "irsdk_TrkSurf")]
    trk_surf: TrackSurface,
    #[serde(rename = "irsdk_SessionState")]
    session_state: SessionState,
    #[serde(rename = "irsdk_CarLeftRight")]
    car_left_right: CarLeftRight,
    #[serde(rename = "irsdk_PitSvStatus")]
    pit_sv_status: PitServiceStatus,
    #[serde(rename = "irsdk_PaceMode")]
    pace_mode: PaceMode,
    #[serde(rename = "irsdk_TrackWetness")]
    track_wetness: TrackWetness,
    #[serde(rename = "irsdk_BroadcastMsg")]
    broadcast_msg: BroadcastMessage,
    #[serde(rename = "irsdk_ChatCommandMode")]
    chat_command_mode: ChatCommandMode,
    #[serde(rename = "irsdk_PitCommandMode")]
    pit_command_mode: PitCommandMode,
    #[serde(rename = "irsdk_TelemetryCommandMode")]
    telemetry_command_mode: TelemetryCommandMode,
    #[serde(rename = "irsdk_RpyStateMode")]
    rpy_state_mode: ReplayStateMode,
    #[serde(rename = "irsdk_ReloadTexturesMode")]
    reload_textures_mode: ReloadTexturesMode,
    #[serde(rename = "irsdk_RpySrchMode")]
    rpy_srch_mode: ReplaySearchMode,
    #[serde(rename = "irsdk_RpyPosMode")]
    rpy_pos_mode: ReplayPositionMode,
    #[serde(rename = "irsdk_FFBCommandMode")]
    ffb_command_mode: ForceFeedbackCommandMode,
    #[serde(rename = "irsdk_csMode")]
    cs_mode: CameraSwitchFocusMode,
    #[serde(rename = "irsdk_VideoCaptureMode")]
    video_capture_mode: VideoCaptureMode,
    #[serde(rename = "irsdk_EngineWarnings")]
    engine_warnings: EngineWarnings,
    #[serde(rename = "irsdk_Flags")]
    flags: SessionFlags,
    #[serde(rename = "irsdk_CameraState")]
    camera_state: CameraState,
    #[serde(rename = "irsdk_PitSvFlags")]
    pit_sv_flags: PitServiceFlags,
    #[serde(rename = "irsdk_PaceFlags")]
    pace_flags: PaceFlags,
    #[serde(rename = "irsdk_IncidentFlags")]
    incident_flags: IncidentFlags,
}

/// CLI arguments for the iRacing primitives schema generator.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the generated schema YAML should be written.
    #[arg(short, long, default_value = "primitives-schema.yml")]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args { output_path } = Args::parse();
    let schema = schemars::schema_for!(IrsdkPrimitivesSchema);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    tracing::info!(
        path = %output_path.display(),
        "Wrote iRacing primitive schema"
    );

    Ok(())
}
