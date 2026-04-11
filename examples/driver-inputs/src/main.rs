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

/// Parse CLI arguments, read frames from an IBT file, adapt them to `DriverInput`, and write each adapted frame as a CSV row to the specified output path.
///
/// The program exits with `Ok(())` on success. It returns an error if schema validation, frame iteration, frame adaptation, CSV serialization, or flushing fails.
///
/// # Examples
///
/// ```no_run
/// // Typical usage: run the compiled binary with `--ibt-path` and `--csv-output-path`.
/// // From code/tests you can invoke the entrypoint directly (no runtime side-effects in this doctest).
/// let _ = crate::main();
/// ```
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        ibt_path,
        csv_output_path,
    } = Args::parse();

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

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

    writer.flush()?;

    Ok(())
}
