use anyhow::Result;
use clap::Parser;
use iracing_sdk::{
    BitField, SchemaProvider, VarData,
    ibt::IbtReader,
    irsdk::{
        CarLeftRight, EngineWarnings, PaceMode, PitServiceFlags, SessionFlags, SessionState,
        TrackLocation, TrackSurface, TrackWetness,
    },
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Decode IRSDK enum/bitfield telemetry from an IBT file"
)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut reader = IbtReader::open(&args.ibt_path)?;
    let schema = reader.schema().clone();

    let session_state = schema.get_variable("SessionState").cloned();
    let session_flags = schema.get_variable("SessionFlags").cloned();
    let player_track_surface = schema.get_variable("PlayerTrackSurface").cloned();
    let player_track_surface_material = schema.get_variable("PlayerTrackSurfaceMaterial").cloned();
    let car_left_right = schema.get_variable("CarLeftRight").cloned();
    let track_wetness = schema.get_variable("TrackWetness").cloned();
    let engine_warnings = schema.get_variable("EngineWarnings").cloned();
    let pace_mode = schema.get_variable("PaceMode").cloned();
    let pit_sv_flags = schema.get_variable("PitSvFlags").cloned();

    while let Some((frame, tick, _session_version)) = reader.read_next_frame()? {
        let session_state_value = session_state
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| SessionState::try_from(v).ok());

        let session_flags_value = session_flags
            .as_ref()
            .and_then(|info| BitField::from_bytes(&frame, info).ok())
            .map(SessionFlags::from);

        let is_caution = session_flags_value
            .map(|f| f.has_any_caution())
            .unwrap_or(false);

        let player_track_surface_value = player_track_surface
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| TrackLocation::try_from(v).ok());

        let player_track_surface_material_value = player_track_surface_material
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| TrackSurface::try_from(v).ok());

        let car_left_right_value = car_left_right
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| CarLeftRight::try_from(v).ok());

        let track_wetness_value = track_wetness
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| TrackWetness::try_from(v).ok());

        let engine_warnings_value = engine_warnings
            .as_ref()
            .and_then(|info| BitField::from_bytes(&frame, info).ok())
            .map(EngineWarnings::from);

        let has_required_repairs = engine_warnings_value
            .map(|f| f.has_mandatory_repair_warning())
            .unwrap_or(false);

        let pace_mode_value = pace_mode
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .map(|v| PaceMode::try_from(v).ok());

        let pit_sv_flags_value = pit_sv_flags
            .as_ref()
            .and_then(|info| BitField::from_bytes(&frame, info).ok())
            .map(PitServiceFlags::from);

        let has_service_request = pit_sv_flags_value
            .map(|f| f.has_any_service())
            .unwrap_or(false);

        tracing::info!(
            "tick={tick} session={session_state_value:?} track_loc={player_track_surface_value:?} track_surf={player_track_surface_material_value:?} lr={car_left_right_value:?} wet={track_wetness_value:?} pace={pace_mode_value:?} caut={is_caution:?} mand_rep={has_required_repairs:?} fast_rep={has_service_request:?}",
        );
    }

    Ok(())
}
