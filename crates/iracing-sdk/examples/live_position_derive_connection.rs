//! Subscribe to typed live telemetry through [`LiveConnection`](iracing_sdk::LiveConnection).
//!
//! This example requires Windows and an active iRacing session.
//!
//! ```text
//! cargo run -p iracing-sdk --example live-subscribe -- --max-frames 120
//! ```

use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::IRacingTelemetryFrame;

#[cfg(windows)]
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long, default_value_t = 120)]
    max_frames: usize,
}

#[cfg(windows)]
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

fn main() -> Result<()> {
    run()
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn run() -> Result<()> {
    use futures::StreamExt;
    use iracing_sdk::{LiveConnection, UpdateRate};

    let Args { max_frames } = Args::parse();
    let connection = LiveConnection::builder().build()?;
    let mut frames = Box::pin(connection.subscribe::<DriverInputs>(UpdateRate::Native)?);

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

#[cfg(not(windows))]
fn run() -> Result<()> {
    anyhow::bail!("live-subscribe requires Windows and an active iRacing session")
}
