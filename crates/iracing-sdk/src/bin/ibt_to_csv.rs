//! IBT to CSV CLI
//!
//! Parses an `.ibt` telemetry file and writes the output to a CSV.
//!
//! This binary is intended for:
//! - Converting an `.ibt` file into a standalone CSV file for sharing.
//!
//! # Behavior
//! - Opens the `.ibt` file using [`iracing_sdk::ibt::IbtReader`]
//! - Streams [`iracing_sdk::DynamicFrame`] values through an
//!   [`iracing_sdk::IbtConnection`]
//! - Reads all variables through the dynamic value API and writes them to a CSV
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
use futures::StreamExt;
use iracing_sdk::{
    DynamicFrame, IbtConnection, TelemetryValue, VariableInfo, ibt::IbtReader,
    providers::ibt::IbtProvider,
};
use std::path::PathBuf;
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
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
    tracing::info!(path = %ibt_path.display(), "Opening IBT file");
    let reader = IbtReader::open(&ibt_path).context("Failed to open IBT file")?;

    // Clone and sort variables for deterministic column ordering.
    let mut variables: Vec<VariableInfo> = reader.variables().variables.values().cloned().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    let provider = IbtProvider::from_reader(reader);
    let connection = IbtConnection::builder()
        .with_provider(provider)
        .build()
        .await?;

    tracing::info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = Writer::from_path(&output_path).context("Could not create CSV output")?;

    let headers = build_headers(&variables);
    let expected_column_count = headers.len();
    writer.write_record(&headers)?;

    tracing::info!(
        variable_count = variables.len(),
        column_count = expected_column_count,
        "Starting CSV export"
    );

    // ------------------------------------------------------------
    // Frame streaming
    // ------------------------------------------------------------
    let mut frame_count = 0usize;
    let mut stream = connection.subscribe::<DynamicFrame>(iracing_sdk::UpdateRate::Native);
    while let Some(frame) = stream.next().await {
        let mut row = Vec::with_capacity(expected_column_count);
        row.push(frame.tick_count().to_string());

        for variable in &variables {
            let value = frame
                .value_from_info(variable)
                .with_context(|| format!("Failed to decode `{}`", variable.name))?;

            append_value(&mut row, value);
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

        if frame_count.is_multiple_of(10_000) {
            tracing::debug!(frames_exported = frame_count, "CSV export progress");
        }
    }

    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished CSV export");

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

fn append_value(row: &mut Vec<String>, value: TelemetryValue) {
    match value {
        TelemetryValue::Char(value) => row.push(char::from(value).to_string()),
        TelemetryValue::Int8(value) => row.push(value.to_string()),
        TelemetryValue::UInt8(value) => row.push(value.to_string()),
        TelemetryValue::Int16(value) => row.push(value.to_string()),
        TelemetryValue::UInt16(value) => row.push(value.to_string()),
        TelemetryValue::Int32(value) => row.push(value.to_string()),
        TelemetryValue::UInt32(value) => row.push(value.to_string()),
        TelemetryValue::Float32(value) => row.push(value.to_string()),
        TelemetryValue::Float64(value) => row.push(value.to_string()),
        TelemetryValue::Bool(value) => row.push(value.to_string()),
        TelemetryValue::BitField(value) => row.push(value.value().to_string()),
        TelemetryValue::Array(values) => {
            for value in values {
                append_value(row, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::VariableType;

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
    fn append_value_formats_scalars_and_flattens_arrays() {
        let mut row = Vec::new();
        append_value(&mut row, TelemetryValue::Float32(10.0));
        append_value(&mut row, TelemetryValue::Bool(true));
        append_value(
            &mut row,
            TelemetryValue::Array(vec![TelemetryValue::UInt32(2), TelemetryValue::UInt32(3)]),
        );

        assert_eq!(row, vec!["10", "true", "2", "3"]);
    }
}
