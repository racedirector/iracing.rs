//! Print frame/time ranges for each indexed player lap in an iRacing `.ibt` file.

use anyhow::{Result, anyhow};
use iracing_sdk::{IbtReader, IndexedIbt};
use std::{env, path::PathBuf};

fn main() -> Result<()> {
    let ibt_path = env::args_os().nth(1).map(PathBuf::from).ok_or_else(|| {
        anyhow!("usage: cargo run --example ibt_lap_ranges -- <path-to-file.ibt>")
    })?;

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
