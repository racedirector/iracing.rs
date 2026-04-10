mod driver_input;

use clap::Parser;
use csv::Writer;
use driver_input::DriverInput;
use iracing_sdk::{FrameAdapter, IbtProvider, Provider};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,

    #[arg(short, long)]
    csv_output_path: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        ibt_path,
        csv_output_path,
    } = Args::parse();

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    // TODO: open the IBT file, adapt `DriverInput` rows, and write the decoded
    // telemetry to the requested output format.
    let mut ibt_provider =
        IbtProvider::from_path(&ibt_path).expect("Failed to initialize IBT provider");
    let schema = ibt_provider.schema();

    info!(
        total_frames = ibt_provider.total_frames(),
        "Parsing frames from IBT provider"
    );

    let shared_validation = DriverInput::validate_schema(&schema)?;
    while let Some(packet) = ibt_provider.next_frame()? {
        let frame = DriverInput::adapt(&packet, &shared_validation);
        // Serialize row to CSV.
        writer.serialize(frame)?;
    }

    Ok(())
}
