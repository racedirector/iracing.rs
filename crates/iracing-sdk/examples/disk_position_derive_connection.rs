//! Subscribe to typed telemetry from an IBT recording.
//!
//! ```text
//! cargo run -p iracing-sdk --example ibt-subscribe -- \
//!   --ibt-path ./session.ibt --max-frames 5
//! ```

use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use iracing_sdk::{IRacingTelemetryFrame, IbtConnection};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,

    #[arg(short, long, default_value_t = 5)]
    max_frames: usize,
}

#[derive(Debug, IRacingTelemetryFrame)]
struct DriverInputs {
    #[field_name = "Speed"]
    #[fail_if_missing]
    speed_mps: f32,

    #[field_name = "Gear"]
    #[fail_if_missing]
    gear: i32,

    #[field_name = "Throttle"]
    throttle: f32,

    #[field_name = "Brake"]
    brake: f32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let Args {
        ibt_path,
        max_frames,
    } = Args::parse();
    let connection = IbtConnection::builder().with_path(ibt_path).build().await?;
    let mut frames = Box::pin(connection.subscribe::<DriverInputs>()?);
    connection.start()?;

    for _ in 0..max_frames {
        let Some(frame) = frames.next().await else {
            break;
        };
        println!(
            "speed_mps={} gear={} throttle={} brake={}",
            frame.speed_mps, frame.gear, frame.throttle, frame.brake
        );
    }

    Ok(())
}
