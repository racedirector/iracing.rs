//! IBT to CSV CLI
//!
//! Parses an `.ibt` telemetry file and writes the output to a CSV.
//!
//! This binary is intended for:
//! - Converting an `.ibt` file into a standalone CSV file for sharing.
//!
//! # Behavior
//! - Opens the `.ibt` file using [`iracing_sdk::IbtReader`]
//! - Extracts "frames" from `reader.read_next_frame()`
//! - Reads all variables from the frame and writes them to a CSV.
//!
//! # Logging
//! Uses `tracing` + `tracing_subscriber`.
//!
//! - Defaults to `trace` level
//! - Override with `RUST_LOG`, e.g. `RUST_LOG=info`
//!
//! # Usage
//!
//! ```text
//! ibt_to_csv --ibt-path <PATH_TO_FILE.ibt> --output-path <OUTPUT_FILE.csv>
//! ```
//!
//! # Examples
//!
//! Parse an `.ibt` file and write CSV:
//!
//! ```bash
//! cargo run -p iracing-sdk --bin ibt_to_csv -- \
//!   --ibt-path "C:\path\to\telemetry.ibt" \
//!   --output-path "C:\path\to\telemetry.csv"
//! ```
//!
//! Reduce log noise:
//!
//! ```bash
//! RUST_LOG=info cargo run -p iracing-sdk --bin ibt_to_csv -- \
//!   -i "./telemetry.ibt" \
//!   -o "./telemetry.csv"
//! ```
//!
//! # Exit codes / errors
//! - Returns an error if the `.ibt` cannot be opened or read
//! - Returns an error if the output file cannot be written
//!

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use csv::Writer;
use iracing_sdk::{IbtReader, VariableInfo, VariableType};
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the session CSV should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    info!(path = %ibt_path.display(), "Opening IBT file");
    let mut reader = IbtReader::open(&ibt_path).context("Failed to open IBT file")?;

    info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = Writer::from_path(&output_path).context("Could not create CSV output")?;

    // Clone and sort variables for deterministic column ordering.
    let mut variables: Vec<VariableInfo> = reader.variables().variables.values().cloned().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    let headers = build_headers(&variables);
    let expected_column_count = headers.len();
    writer.write_record(&headers)?;

    info!(
        variable_count = variables.len(),
        column_count = expected_column_count,
        "Starting CSV export"
    );

    // ------------------------------------------------------------
    // Frame iteration
    // ------------------------------------------------------------
    //
    // `read_next_frame()` returns:
    //   Result<Option<(data, tick, session_version)>>
    //
    // - Err(_)       => read failure
    // - Ok(None)     => end-of-stream
    // - Ok(Some(...))=> next frame
    //
    let mut frame_count = 0usize;
    while let Some((frame, tick, session_version)) = reader.read_next_frame()? {
        let mut row = Vec::with_capacity(expected_column_count);
        row.push(tick.to_string());
        row.push(session_version.to_string());

        for variable in &variables {
            append_variable_values(&mut row, &frame, variable)?;
        }

        if row.len() != expected_column_count {
            return Err(anyhow!(
                "Internal CSV row width mismatch: expected {} columns, found {}",
                expected_column_count,
                row.len()
            ));
        }

        writer.write_record(&row)?;
        frame_count += 1;

        if frame_count % 10_000 == 0 {
            debug!(frames_exported = frame_count, "CSV export progress");
        }
    }

    writer.flush()?;
    info!(frames_exported = frame_count, "Finished CSV export");

    Ok(())
}

fn build_headers(variables: &[VariableInfo]) -> Vec<String> {
    let mut headers = Vec::with_capacity(2 + expanded_column_count(variables));
    headers.push("tick".to_string());
    headers.push("session_version".to_string());

    for variable in variables {
        if variable.count <= 1 {
            headers.push(variable.name.clone());
            continue;
        }

        for index in 0..variable.count {
            headers.push(format!("{}[{}]", variable.name, index));
        }
    }

    headers
}

fn expanded_column_count(variables: &[VariableInfo]) -> usize {
    variables
        .iter()
        .map(|variable| {
            if variable.count == 0 {
                1
            } else {
                variable.count
            }
        })
        .sum()
}

fn append_variable_values(
    row: &mut Vec<String>,
    frame: &[u8],
    variable: &VariableInfo,
) -> Result<()> {
    if variable.count <= 1 {
        row.push(read_scalar_as_string(frame, variable, 0)?);
        return Ok(());
    }

    for index in 0..variable.count {
        row.push(read_scalar_as_string(frame, variable, index)?);
    }

    Ok(())
}

fn read_scalar_as_string(frame: &[u8], variable: &VariableInfo, index: usize) -> Result<String> {
    let label = variable_label(variable, index);
    let offset = variable_offset(variable, index)?;

    let value = match variable.data_type {
        VariableType::Char => {
            let byte = read_bytes(frame, offset, 1, &label)?[0];
            char::from(byte).to_string()
        }
        VariableType::Int8 => {
            let value = i8::from_le_bytes([read_bytes(frame, offset, 1, &label)?[0]]);
            value.to_string()
        }
        VariableType::UInt8 => {
            let value = read_bytes(frame, offset, 1, &label)?[0];
            value.to_string()
        }
        VariableType::Int16 => {
            let bytes = read_bytes(frame, offset, 2, &label)?;
            i16::from_le_bytes([bytes[0], bytes[1]]).to_string()
        }
        VariableType::UInt16 => {
            let bytes = read_bytes(frame, offset, 2, &label)?;
            u16::from_le_bytes([bytes[0], bytes[1]]).to_string()
        }
        VariableType::Int32 => {
            let bytes = read_bytes(frame, offset, 4, &label)?;
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        VariableType::UInt32 => {
            let bytes = read_bytes(frame, offset, 4, &label)?;
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        VariableType::Float32 => {
            let bytes = read_bytes(frame, offset, 4, &label)?;
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        VariableType::Float64 => {
            let bytes = read_bytes(frame, offset, 8, &label)?;
            f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
            .to_string()
        }
        VariableType::Bool => {
            let value = read_bytes(frame, offset, 1, &label)?[0] != 0;
            value.to_string()
        }
        VariableType::BitField => {
            let bytes = read_bytes(frame, offset, 4, &label)?;
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
    };

    Ok(value)
}

fn variable_offset(variable: &VariableInfo, index: usize) -> Result<usize> {
    let element_size = variable.data_type.size();
    let offset_delta = index
        .checked_mul(element_size)
        .ok_or_else(|| anyhow!("Offset overflow while reading `{}`", variable.name))?;

    variable
        .offset
        .checked_add(offset_delta)
        .ok_or_else(|| anyhow!("Offset overflow while reading `{}`", variable.name))
}

fn read_bytes<'a>(frame: &'a [u8], offset: usize, width: usize, label: &str) -> Result<&'a [u8]> {
    let end = offset
        .checked_add(width)
        .ok_or_else(|| anyhow!("Offset overflow while reading `{label}`"))?;

    frame.get(offset..end).with_context(|| {
        format!(
            "Frame too small while reading `{label}`: offset={offset}, width={width}, frame_len={}",
            frame.len()
        )
    })
}

fn variable_label(variable: &VariableInfo, index: usize) -> String {
    if variable.count <= 1 {
        return variable.name.clone();
    }

    format!("{}[{}]", variable.name, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(name: &str, data_type: VariableType, offset: usize, count: usize) -> VariableInfo {
        VariableInfo {
            name: name.to_string(),
            data_type,
            offset,
            count,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn build_headers_expands_array_variables() {
        let variables = vec![
            variable("Speed", VariableType::Float32, 0, 1),
            variable("CarIdxLapDistPct", VariableType::Float32, 4, 3),
        ];

        let headers = build_headers(&variables);
        assert_eq!(
            headers,
            vec![
                "tick",
                "session_version",
                "Speed",
                "CarIdxLapDistPct[0]",
                "CarIdxLapDistPct[1]",
                "CarIdxLapDistPct[2]",
            ]
        );
    }

    #[test]
    fn append_variable_values_reads_scalars_and_arrays() -> Result<()> {
        let frame = vec![
            0x00, 0x00, 0x20, 0x41, // Speed = 10.0f32
            0x01, // OnPitRoad = true
            0x02, 0x00, 0x00, 0x00, // Flags[0] = 2
            0x03, 0x00, 0x00, 0x00, // Flags[1] = 3
        ];

        let speed = variable("Speed", VariableType::Float32, 0, 1);
        let on_pit_road = variable("OnPitRoad", VariableType::Bool, 4, 1);
        let flags = variable("Flags", VariableType::UInt32, 5, 2);

        let mut row = Vec::new();
        append_variable_values(&mut row, &frame, &speed)?;
        append_variable_values(&mut row, &frame, &on_pit_road)?;
        append_variable_values(&mut row, &frame, &flags)?;

        assert_eq!(row, vec!["10", "true", "2", "3"]);
        Ok(())
    }
}
