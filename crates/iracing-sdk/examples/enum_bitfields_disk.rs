use anyhow::Result;
use clap::Parser;
use futures::executor::block_on;
use iracing_sdk::provider::Provider;
use iracing_sdk::{
    BitField, LayoutProvider, VarData,
    irsdk::{
        CarLeftRight, EngineWarnings, PaceMode, PitServiceFlags, SessionFlags, SessionState,
        TrackLocation, TrackSurface, TrackWetness,
    },
    providers::ibt::IbtProvider,
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
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    let mut reader = IbtProvider::open(&args.ibt_path)?;
    let layout = reader.layout().clone();

    let session_state = layout
        .field_by_name("SessionState")
        .map(|(_, field)| field)
        .cloned();
    let session_flags = layout
        .field_by_name("SessionFlags")
        .map(|(_, field)| field)
        .cloned();
    let player_track_surface = layout
        .field_by_name("PlayerTrackSurface")
        .map(|(_, field)| field)
        .cloned();
    let player_track_surface_material = layout
        .field_by_name("PlayerTrackSurfaceMaterial")
        .map(|(_, field)| field)
        .cloned();
    let car_left_right = layout
        .field_by_name("CarLeftRight")
        .map(|(_, field)| field)
        .cloned();
    let track_wetness = layout
        .field_by_name("TrackWetness")
        .map(|(_, field)| field)
        .cloned();
    let engine_warnings = layout
        .field_by_name("EngineWarnings")
        .map(|(_, field)| field)
        .cloned();
    let pace_mode = layout
        .field_by_name("PaceMode")
        .map(|(_, field)| field)
        .cloned();
    let pit_sv_flags = layout
        .field_by_name("PitSvFlags")
        .map(|(_, field)| field)
        .cloned();

    while let Some(packet) = block_on(reader.next_frame())? {
        let frame = packet.data();
        let tick = packet.tick;
        let session_state_value = session_state
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| SessionState::try_from(v).ok());

        let session_flags_value = session_flags
            .as_ref()
            .and_then(|info| BitField::decode_field(frame, info).ok())
            .map(SessionFlags::from);

        let is_caution = session_flags_value
            .map(|f| f.has_any_caution())
            .unwrap_or(false);

        let player_track_surface_value = player_track_surface
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| TrackLocation::try_from(v).ok());

        let player_track_surface_material_value = player_track_surface_material
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| TrackSurface::try_from(v).ok());

        let car_left_right_value = car_left_right
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| CarLeftRight::try_from(v).ok());

        let track_wetness_value = track_wetness
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| TrackWetness::try_from(v).ok());

        let engine_warnings_value = engine_warnings
            .as_ref()
            .and_then(|info| BitField::decode_field(frame, info).ok())
            .map(EngineWarnings::from);

        let has_required_repairs = engine_warnings_value
            .map(|f| f.has_mandatory_repair_warning())
            .unwrap_or(false);

        let pace_mode_value = pace_mode
            .as_ref()
            .and_then(|info| i32::decode_field(frame, info).ok())
            .map(|v| PaceMode::try_from(v).ok());

        let pit_sv_flags_value = pit_sv_flags
            .as_ref()
            .and_then(|info| BitField::decode_field(frame, info).ok())
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
