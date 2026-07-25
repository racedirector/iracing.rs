//! Live telemetry to CSV CLI (Windows)
//!
//! Connects to a running iRacing instance and writes telemetry frames to a CSV.
//!
//! This binary mirrors `ibt_to_csv` CSV formatting:
//! - Columns are deterministic and expanded for array variables
//! - The `tick` column is emitted first
//! - Every telemetry variable in the live schema is written per frame
//!
//! # Platform
//! This tool relies on `iracing_sdk::WindowsConnection`, so it is only usable on
//! Windows with iRacing shared memory available.
//!
//! # Usage
//!
//! ```text
//! live_to_csv --output-path <OUTPUT_FILE.csv>
//! ```

#[cfg(any(windows, test))]
use anyhow::Context;
use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use csv::Writer;
#[cfg(windows)]
use futures::StreamExt;
use iracing_sdk::TelemetryValue;
#[cfg(any(windows, test))]
use iracing_sdk::VariableInfo;
#[cfg(windows)]
use iracing_sdk::{DynamicFrame, LiveConnection, WindowsConnection, providers::live::LiveProvider};
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the telemetry CSV should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn run() -> Result<()> {
    let Args { output_path } = Args::parse();

    tracing::info!("Opening iRacing connection");
    let connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    // Sort the variables for extraction by the offset of each variable info
    let mut variables = connection.get_variables();
    if variables.is_empty() {
        return Err(anyhow!(
            "No telemetry variables were available from the live connection"
        ));
    }

    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    // Create a provider for actual extraction
    let provider = LiveProvider::builder()
        .with_connection(connection)
        .build()?;

    let connection = LiveConnection::builder().with_provider(provider).build()?;

    tracing::info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = Writer::from_path(&output_path).context("Could not create CSV output")?;

    let headers = build_headers(&variables);
    let expected_column_count = headers.len();
    writer.write_record(&headers)?;

    tracing::info!(
        variable_count = variables.len(),
        column_count = expected_column_count,
        "Starting live CSV export"
    );

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
    }

    // Clean up
    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished live CSV export");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_csv is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live_to_csv is only supported on Windows"))
}

#[cfg(any(windows, test))]
fn build_headers(variables: &[VariableInfo]) -> Vec<String> {
    let mut headers = Vec::with_capacity(2 + expanded_column_count(variables));
    headers.push("tick".to_string());

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

#[cfg(any(windows, test))]
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
