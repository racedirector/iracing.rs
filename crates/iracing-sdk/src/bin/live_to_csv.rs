//! Live telemetry to CSV CLI (Windows)
//!
//! Connects to a running iRacing instance and writes telemetry frames to a CSV.
//!
//! This binary mirrors `ibt_to_csv` CSV formatting:
//! - Columns are deterministic and expanded for array variables
//! - `tick` and `session_version` columns are emitted first
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
#[cfg(any(windows, test))]
use iracing_sdk::{VariableInfo, VariableType};
#[cfg(windows)]
use iracing_sdk::{WaitResult, WindowsConnection};
#[cfg(windows)]
use std::{path::PathBuf, time::Duration};
#[cfg(windows)]
use tracing::{debug, info, trace};
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
fn run() -> Result<()> {
    let Args { output_path } = Args::parse();

    info!("Opening iRacing connection");
    let mut connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = Writer::from_path(&output_path).context("Could not create CSV output")?;

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

    let headers = build_headers(&variables);
    let expected_column_count = headers.len();
    writer.write_record(&headers)?;

    info!(
        variable_count = variables.len(),
        column_count = expected_column_count,
        "Starting live CSV export"
    );

    let mut frame_count = 0usize;
    loop {
        if !connection.is_connected() {
            info!("iRacing disconnected; stopping live CSV export");
            break;
        }

        if let Some(raw_frame) = connection.get_new_data() {
            let frame = raw_frame.to_vec();
            let tick = latest_tick(&connection);
            let session_version = connection.session_info_update();

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

        match connection.wait_for_update(Duration::from_millis(500))? {
            WaitResult::Signaled => {
                trace!("Telemetry update signaled");
            }
            WaitResult::Timeout => {
                trace!("Wait timeout while polling live telemetry");
            }
        }
    }

    writer.flush()?;
    info!(frames_exported = frame_count, "Finished live CSV export");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_csv is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live_to_csv is only supported on Windows"))
}

#[cfg(windows)]
fn latest_tick(connection: &WindowsConnection) -> i32 {
    let header = connection.header();
    let latest_idx = connection.find_latest_buffer(header);
    header.var_buf[latest_idx].tick_count
}

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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
