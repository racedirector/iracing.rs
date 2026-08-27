use anyhow::Result;
use clap::Parser;
use iracing_sdk::{
    VarData, VariableSchema,
    reader::ibt::IbtReader,
    types::irsdk::{
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
    let schema = VariableSchema::from_reader(&reader)?;

    let session_state = schema.get_variable("SessionState").cloned();
    let session_flags = schema.get_variable("SessionFlags").cloned();
    let player_track_surface = schema.get_variable("PlayerTrackSurface").cloned();
    let player_track_surface_material = schema.get_variable("PlayerTrackSurfaceMaterial").cloned();
    let car_left_right = schema.get_variable("CarLeftRight").cloned();
    let track_wetness = schema.get_variable("TrackWetness").cloned();
    let engine_warnings = schema.get_variable("EngineWarnings").cloned();
    let pace_mode = schema.get_variable("PaceMode").cloned();
    let pit_sv_flags = schema.get_variable("PitSvFlags").cloned();

    while let Some(frame) = reader.read_next_frame()? {
        let tick = frame.index();
        let frame: Vec<u8> = frame.into_buffer().into();
        let session_state_value = session_state
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| SessionState::try_from(raw).ok());

        let session_flags_value = session_flags
            .as_ref()
            .and_then(|info| iracing_sdk::BitField::from_bytes(&frame, info).ok())
            .map(SessionFlags::from);

        let player_track_surface_value = player_track_surface
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| TrackLocation::try_from(raw).ok());

        let player_track_surface_material_value = player_track_surface_material
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| TrackSurface::try_from(raw).ok());

        let car_left_right_value = car_left_right
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| CarLeftRight::try_from(raw).ok());

        let track_wetness_value = track_wetness
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| TrackWetness::try_from(raw).ok());

        let engine_warnings_value = engine_warnings
            .as_ref()
            .and_then(|info| iracing_sdk::BitField::from_bytes(&frame, info).ok())
            .map(EngineWarnings::from);

        let pace_mode_value = pace_mode
            .as_ref()
            .and_then(|info| i32::from_bytes(&frame, info).ok())
            .and_then(|raw| PaceMode::try_from(raw).ok());

        let pit_sv_flags_value = pit_sv_flags
            .as_ref()
            .and_then(|info| iracing_sdk::BitField::from_bytes(&frame, info).ok())
            .map(PitServiceFlags::from);

        println!(
            "tick={tick} session={session_state_value:?} track_loc={player_track_surface_value:?} track_surf={player_track_surface_material_value:?} lr={car_left_right_value:?} wet={track_wetness_value:?} pace={pace_mode_value:?} caut={} mand_rep={} fast_rep={}",
            session_flags_value
                .map(|f| f.contains(SessionFlags::CAUTION))
                .unwrap_or(false),
            engine_warnings_value
                .map(|f| f.contains(EngineWarnings::MANDATORY_REPAIR_NEEDED))
                .unwrap_or(false),
            pit_sv_flags_value
                .map(|f| f.contains(PitServiceFlags::FAST_REPAIR))
                .unwrap_or(false),
        );
    }

    Ok(())
}
