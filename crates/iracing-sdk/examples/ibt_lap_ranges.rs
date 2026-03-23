//! # ibt-lap-ranges
//!
//! Prints frame and session-time ranges for each indexed player lap in an iRacing `.ibt` file.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example ibt_lap_ranges -- --ibt-path ./session.ibt
//! ```

use anyhow::Result;
use clap::Parser;
use iracing_sdk::{IbtReader, IndexedIbt};
use std::path::PathBuf;

/// CLI arguments for the `ibt-lap-ranges` example.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,
}

fn main() -> Result<()> {
    let Args { ibt_path } = Args::parse();

    let mut reader = IbtReader::open(&ibt_path)?;
    let indexed = IndexedIbt::build(&mut reader)?;

    println!(
        "Indexed {} lap slices from {}",
        indexed.lap_count(),
        ibt_path.display()
    );
    for lap in indexed.index().laps.iter() {
        println!(
            "lap #{:>2} number={:?} frames={}..={} time={:?}..={:?} partial_first={} partial_last={} pit={}",
            lap.ordinal,
            lap.lap_number,
            lap.start_frame,
            lap.end_frame,
            lap.start_session_time,
            lap.end_session_time,
            lap.flags.partial_first_lap,
            lap.flags.partial_last_lap,
            lap.flags.touches_pit_road,
        );
    }

    Ok(())
}
