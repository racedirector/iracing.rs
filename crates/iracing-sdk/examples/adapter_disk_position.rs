//! Implement and run a manual [`FrameAdapter`](iracing_sdk::FrameAdapter).
//!
//! ```text
//! cargo run -p iracing-sdk --example manual-frame-adapter -- \
//!   --ibt-path ./session.ibt --max-frames 5
//! ```

use anyhow::Result;
use clap::Parser;
use iracing_sdk::{
    AdapterValidation, FieldExtraction, FrameAdapter, IRacingSDKError, SchemaProvider,
    VariableSchema, provider::Provider, providers::ibt::IbtProvider,
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

#[derive(Debug)]
struct DriverInputs {
    speed_mps: f32,
    gear: i32,
    throttle: f32,
    brake: f32,
}

impl FrameAdapter for DriverInputs {
    fn validate_schema(schema: &VariableSchema) -> iracing_sdk::Result<AdapterValidation> {
        let extraction_plan = ["Speed", "Gear", "Throttle", "Brake"]
            .into_iter()
            .map(|name| {
                let var_info = schema
                    .get_variable(name)
                    .ok_or_else(|| IRacingSDKError::Parse {
                        context: "DriverInputs schema validation".to_string(),
                        details: format!("missing required field `{name}`"),
                    })?;

                Ok(FieldExtraction::Required {
                    name: name.to_string(),
                    var_info: var_info.clone(),
                })
            })
            .collect::<iracing_sdk::Result<Vec<_>>>()?;

        Ok(AdapterValidation::new(extraction_plan))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        Self {
            speed_mps: validation.fetch_or_default(packet, "Speed"),
            gear: validation.fetch_or_default(packet, "Gear"),
            throttle: validation.fetch_or_default(packet, "Throttle"),
            brake: validation.fetch_or_default(packet, "Brake"),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let Args {
        ibt_path,
        max_frames,
    } = Args::parse();
    let mut provider = IbtProvider::open(ibt_path)?;
    let validation = DriverInputs::validate_schema(provider.schema())?;

    for _ in 0..max_frames {
        let Some(packet) = provider.next_frame().await? else {
            break;
        };
        let inputs = DriverInputs::adapt(&packet, &validation);
        println!(
            "tick={} speed_mps={} gear={} throttle={} brake={}",
            packet.tick, inputs.speed_mps, inputs.gear, inputs.throttle, inputs.brake
        );
    }

    Ok(())
}
