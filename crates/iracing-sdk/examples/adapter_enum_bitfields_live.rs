use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::types::irsdk as sdk;
#[cfg(windows)]
use iracing_sdk::{
    AdapterValidation, FieldExtraction, FrameAdapter, SchemaProvider, providers::live::LiveProvider,
};
#[cfg(windows)]
use iracing_sdk::{BitField, IRacingSDKError, VarData};

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about = "Decode enum/bitfield telemetry via FrameAdapter")]
struct Args {
    #[arg(short, long, default_value_t = 120)]
    max_frames: usize,
}

#[cfg(windows)]
#[derive(Debug)]
struct TelemetryRow {
    session_state: sdk::SessionState,
    session_flags: sdk::SessionFlags,
    track_surface: sdk::TrackSurface,
    engine_warnings: sdk::EngineWarnings,
}

#[cfg(windows)]
impl FrameAdapter for TelemetryRow {
    fn validate_schema(
        schema: &iracing_sdk::VariableSchema,
    ) -> iracing_sdk::Result<AdapterValidation> {
        let mut extraction_plan = Vec::new();

        for required in [
            "SessionState",
            "SessionFlags",
            "PlayerTrackSurfaceMaterial",
            "EngineWarnings",
        ] {
            let var_info = schema
                .get_variable(required)
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Field validation".to_string(),
                    details: format!("Missing required field '{required}'"),
                })?
                .clone();

            extraction_plan.push(FieldExtraction::Required {
                name: required.to_string(),
                var_info,
            });
        }

        Ok(AdapterValidation::new(extraction_plan))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        let fetch_bitfield = |name: &str| {
            validation
                .index_of(name)
                .and_then(|idx| validation.extraction_plan.get(idx))
                .and_then(|field| field.var_info())
                .and_then(|info| BitField::from_bytes(packet.data.as_ref(), info).ok())
                .unwrap_or_else(|| BitField::new(0))
        };

        let session_state_raw = validation.fetch_or_default::<i32>(packet, "SessionState");
        let track_surface_raw =
            validation.fetch_or_default::<i32>(packet, "PlayerTrackSurfaceMaterial");
        let session_flags_raw = fetch_bitfield("SessionFlags");
        let engine_warnings_raw = fetch_bitfield("EngineWarnings");

        Self {
            session_state: sdk::SessionState::try_from(session_state_raw)
                .unwrap_or(sdk::SessionState::Invalid),
            session_flags: sdk::SessionFlags::from(session_flags_raw),
            track_surface: sdk::TrackSurface::try_from(track_surface_raw)
                .unwrap_or(sdk::TrackSurface::SurfaceNotInWorld),
            engine_warnings: sdk::EngineWarnings::from(engine_warnings_raw),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    #[cfg(not(windows))]
    {
        Err(anyhow::anyhow!(
            "enum-bitfields-live example is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use iracing_sdk::provider::Provider;

        let args = Args::parse();
        let mut provider = LiveProvider::new()?;
        let validation = TelemetryRow::validate_schema(provider.schema())?;

        let mut seen = 0usize;
        while let Some(packet) = provider.next_frame().await? {
            if seen >= args.max_frames {
                break;
            }

            let row = TelemetryRow::adapt(&packet, &validation);

            println!(
                "tick={} state={:?} track={:?} caution={} mandatory_repair={}",
                packet.tick,
                row.session_state,
                row.track_surface,
                row.session_flags.contains(sdk::SessionFlags::CAUTION),
                row.engine_warnings
                    .contains(sdk::EngineWarnings::MANDATORY_REPAIR_NEEDED),
            );

            seen += 1;
        }

        Ok(())
    }
}
