//! Decode SDK enums and bitfields from recorded telemetry.
//!
//! ```text
//! cargo run -p iracing-sdk --example enum-bitfields-ibt -- \
//!   --ibt-path ./session.ibt --max-frames 5
//! ```

use anyhow::Result;
use clap::Parser;
use iracing_sdk::{
    BitField, SchemaProvider, VarData,
    irsdk::{EngineWarnings, SessionFlags, SessionState, TrackSurface},
    provider::Provider,
    providers::ibt::IbtProvider,
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,

    #[arg(short, long, default_value_t = 5)]
    max_frames: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let Args {
        ibt_path,
        max_frames,
    } = Args::parse();
    let mut provider = IbtProvider::open(ibt_path)?;

    let session_state = provider.schema().get_variable("SessionState").cloned();
    let session_flags = provider.schema().get_variable("SessionFlags").cloned();
    let track_surface = provider
        .schema()
        .get_variable("PlayerTrackSurfaceMaterial")
        .cloned();
    let engine_warnings = provider.schema().get_variable("EngineWarnings").cloned();

    for _ in 0..max_frames {
        let Some(packet) = provider.next_frame().await? else {
            break;
        };

        let state = session_state
            .as_ref()
            .and_then(|info| SessionState::from_bytes(packet.data.as_ref(), info).ok());
        let surface = track_surface
            .as_ref()
            .and_then(|info| TrackSurface::from_bytes(packet.data.as_ref(), info).ok());
        let flags: Option<SessionFlags> = session_flags
            .as_ref()
            .and_then(|info| BitField::from_bytes(packet.data.as_ref(), info).ok())
            .map(SessionFlags::from);
        let warnings: Option<EngineWarnings> = engine_warnings
            .as_ref()
            .and_then(|info| BitField::from_bytes(packet.data.as_ref(), info).ok())
            .map(EngineWarnings::from);

        println!(
            "tick={} state={state:?} surface={surface:?} caution={} mandatory_repair={}",
            packet.tick,
            flags.is_some_and(SessionFlags::has_any_caution),
            warnings.is_some_and(EngineWarnings::has_mandatory_repair_warning),
        );
    }

    Ok(())
}
