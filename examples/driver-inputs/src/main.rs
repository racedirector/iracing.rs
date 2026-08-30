mod driver_input;

use clap::Parser;
use csv::Writer;
use driver_input::DriverInput;
use iracing_sdk::types::irsdk::SessionFlags;
use iracing_sdk::{FrameAdapter, SchemaProvider, provider::Provider, providers::ibt::IbtProvider};
use std::{fs::File, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,

    #[arg(short, long)]
    csv_output_dir: PathBuf,
}

struct FlagObserver {
    previous_flags: Option<SessionFlags>,
    writer: Writer<File>,
}

impl FlagObserver {
    pub fn new(output_path: PathBuf) -> Self {
        let flag_writer =
            Writer::from_path(output_path).expect("Could not create flags CSV output");

        Self {
            previous_flags: None,
            writer: flag_writer,
        }
    }

    pub fn observe(&mut self, flags: SessionFlags) -> Result<(), Box<dyn std::error::Error>> {
        match self.previous_flags {
            None => self.writer.serialize(flags)?,
            Some(prev_flags) if prev_flags != flags => self.handle_flag_change(flags)?,
            Some(_) => {}
        }

        self.previous_flags = Some(flags);

        Ok(())
    }

    fn handle_flag_change(
        &mut self,
        flags: SessionFlags,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("{:?}", flags.names());

        self.writer.serialize(flags)?;

        Ok(())
    }

    pub fn finalize(&mut self) {
        let _ = self.writer.flush();
    }
}

impl Drop for FlagObserver {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
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
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        csv_output_dir,
    } = Args::parse();

    assert!(
        csv_output_dir.is_dir(),
        "csv_output_dir must be a directory."
    );

    let mut frame_output_path = csv_output_dir.clone();
    frame_output_path.push("output.csv");
    let mut writer = Writer::from_path(frame_output_path).expect("Could not create CSV output");

    let mut flag_csv_output_path = csv_output_dir.clone();
    flag_csv_output_path.push("flags.csv");
    let mut flag_observer = FlagObserver::new(flag_csv_output_path);

    let mut ibt_provider = IbtProvider::open(&ibt_path).expect("Failed to initialize IBT provider");
    let schema = ibt_provider.schema();

    tracing::info!(
        total_frames = ibt_provider.total_frames(),
        "Parsing frames from IBT provider"
    );

    let shared_validation = DriverInput::validate_schema(schema)?;
    while let Some(packet) = ibt_provider.next_frame().await? {
        let frame = DriverInput::adapt(&packet, &shared_validation);
        flag_observer.observe(frame.flags)?;

        // Serialize row to CSV.
        writer.serialize(frame)?;
    }

    flag_observer.finalize();
    writer.flush()?;

    Ok(())
}
