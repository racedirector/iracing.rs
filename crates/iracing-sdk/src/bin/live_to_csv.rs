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
//! live-to-csv --output-path <OUTPUT_FILE.csv>
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

    #[test]
    fn expanded_column_count_handles_zero_count() {
        let variables = vec![
            variable("Speed", VariableType::Float32, 0, 0),
            variable("RPM", VariableType::Float32, 4, 1),
        ];

        let count = expanded_column_count(&variables);
        assert_eq!(count, 2); // Both should count as 1
    }

    #[test]
    fn expanded_column_count_sums_array_elements() {
        let variables = vec![
            variable("Speed", VariableType::Float32, 0, 1),
            variable("CarIdxLapDistPct", VariableType::Float32, 4, 64),
            variable("Gear", VariableType::Int32, 260, 1),
        ];

        let count = expanded_column_count(&variables);
        assert_eq!(count, 66); // 1 + 64 + 1
    }

    #[test]
    fn variable_label_returns_name_for_scalar() {
        let var = variable("Speed", VariableType::Float32, 0, 1);
        assert_eq!(variable_label(&var, 0), "Speed");
    }

    #[test]
    fn variable_label_returns_indexed_name_for_array() {
        let var = variable("CarIdxLapDistPct", VariableType::Float32, 0, 64);
        assert_eq!(variable_label(&var, 0), "CarIdxLapDistPct[0]");
        assert_eq!(variable_label(&var, 32), "CarIdxLapDistPct[32]");
        assert_eq!(variable_label(&var, 63), "CarIdxLapDistPct[63]");
    }

    #[test]
    fn variable_offset_calculates_correct_offset_for_arrays() -> Result<()> {
        let var = variable("CarIdxLapDistPct", VariableType::Float32, 100, 3);

        assert_eq!(variable_offset(&var, 0)?, 100);
        assert_eq!(variable_offset(&var, 1)?, 104); // Float32 is 4 bytes
        assert_eq!(variable_offset(&var, 2)?, 108);

        Ok(())
    }

    #[test]
    fn variable_offset_handles_different_type_sizes() -> Result<()> {
        let var_f64 = variable("SessionTime", VariableType::Float64, 0, 2);
        assert_eq!(variable_offset(&var_f64, 0)?, 0);
        assert_eq!(variable_offset(&var_f64, 1)?, 8); // Float64 is 8 bytes

        let var_i32 = variable("Gear", VariableType::Int32, 50, 3);
        assert_eq!(variable_offset(&var_i32, 0)?, 50);
        assert_eq!(variable_offset(&var_i32, 2)?, 58); // Int32 is 4 bytes

        Ok(())
    }

    #[test]
    fn read_scalar_as_string_handles_all_types() -> Result<()> {
        let frame = vec![
            b'A', // Char
            0xFF, // Int8 = -1
            0x42, // UInt8 = 66
            0x10, 0x00, // Int16 = 16
            0xFF, 0x0F, // UInt16 = 4095
            0x00, 0x10, 0x00, 0x00, // Int32 = 4096
            0xFF, 0xFF, 0x00, 0x00, // UInt32 = 65535
            0x00, 0x00, 0xC8, 0x42, // Float32 = 100.0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x59, 0x40, // Float64 = 100.0
            0x01, // Bool = true
            0x05, 0x00, 0x00, 0x00, // BitField = 5
        ];

        let tests = vec![
            (variable("c", VariableType::Char, 0, 1), "A"),
            (variable("i8", VariableType::Int8, 1, 1), "-1"),
            (variable("u8", VariableType::UInt8, 2, 1), "66"),
            (variable("i16", VariableType::Int16, 3, 1), "16"),
            (variable("u16", VariableType::UInt16, 5, 1), "4095"),
            (variable("i32", VariableType::Int32, 7, 1), "4096"),
            (variable("u32", VariableType::UInt32, 11, 1), "65535"),
            (variable("f32", VariableType::Float32, 15, 1), "100"),
            (variable("f64", VariableType::Float64, 19, 1), "100"),
            (variable("bool", VariableType::Bool, 27, 1), "true"),
            (variable("bitfield", VariableType::BitField, 28, 1), "5"),
        ];

        for (var, expected) in tests {
            let result = read_scalar_as_string(&frame, &var, 0)?;
            assert_eq!(result, expected, "Failed for type {:?}", var.data_type);
        }

        Ok(())
    }

    #[test]
    fn read_scalar_as_string_handles_false_bool() -> Result<()> {
        let frame = vec![0x00]; // false
        let var = variable("OnPitRoad", VariableType::Bool, 0, 1);
        let result = read_scalar_as_string(&frame, &var, 0)?;
        assert_eq!(result, "false");
        Ok(())
    }

    #[test]
    fn read_bytes_returns_error_on_overflow() {
        let frame = vec![0u8; 10];
        let result = read_bytes(&frame, 8, 4, "test");
        assert!(result.is_err());
    }

    #[test]
    fn read_bytes_returns_error_on_offset_overflow() {
        let frame = vec![0u8; 10];
        let result = read_bytes(&frame, usize::MAX - 1, 10, "test");
        assert!(result.is_err());
    }

    #[test]
    fn variable_offset_returns_error_on_multiplication_overflow() {
        let var = variable("Test", VariableType::Float64, 0, usize::MAX);
        let result = variable_offset(&var, usize::MAX);
        assert!(result.is_err());
    }

    #[test]
    fn variable_offset_returns_error_on_addition_overflow() {
        let var = variable("Test", VariableType::Float32, usize::MAX - 1, 1);
        let result = variable_offset(&var, 1);
        assert!(result.is_err());
    }

    #[test]
    fn build_headers_handles_empty_variables() {
        let variables = vec![];
        let headers = build_headers(&variables);
        assert_eq!(headers, vec!["tick", "session_version"]);
    }

    #[test]
    fn build_headers_handles_single_variable() {
        let variables = vec![variable("Speed", VariableType::Float32, 0, 1)];
        let headers = build_headers(&variables);
        assert_eq!(headers, vec!["tick", "session_version", "Speed"]);
    }

    #[test]
    fn append_variable_values_handles_zero_count_as_scalar() -> Result<()> {
        let frame = vec![0x00, 0x00, 0x20, 0x41]; // 10.0f32
        let var = variable("Speed", VariableType::Float32, 0, 0);

        let mut row = Vec::new();
        append_variable_values(&mut row, &frame, &var)?;

        assert_eq!(row, vec!["10"]);
        Ok(())
    }

    #[test]
    fn read_scalar_as_string_handles_negative_integers() -> Result<()> {
        let frame = vec![
            0xFE, // Int8 = -2
            0x00, 0x00, 0x00, 0x00,
            0xFE, 0xFF, // Int16 = -2
            0x00, 0x00,
            0xFE, 0xFF, 0xFF, 0xFF, // Int32 = -2
        ];

        let i8_var = variable("i8", VariableType::Int8, 0, 1);
        let i16_var = variable("i16", VariableType::Int16, 5, 1);
        let i32_var = variable("i32", VariableType::Int32, 9, 1);

        assert_eq!(read_scalar_as_string(&frame, &i8_var, 0)?, "-2");
        assert_eq!(read_scalar_as_string(&frame, &i16_var, 0)?, "-2");
        assert_eq!(read_scalar_as_string(&frame, &i32_var, 0)?, "-2");

        Ok(())
    }

    #[test]
    fn read_scalar_as_string_handles_special_float_values() -> Result<()> {
        let mut frame = Vec::new();
        frame.extend_from_slice(&0.0f32.to_le_bytes()); // 0.0
        frame.extend_from_slice(&(-0.0f32).to_le_bytes()); // -0.0
        frame.extend_from_slice(&f32::INFINITY.to_le_bytes()); // inf
        frame.extend_from_slice(&f32::NEG_INFINITY.to_le_bytes()); // -inf

        let var = variable("f32", VariableType::Float32, 0, 1);

        let zero = read_scalar_as_string(&frame, &var, 0)?;
        assert_eq!(zero, "0");

        let neg_zero_var = variable("f32", VariableType::Float32, 4, 1);
        let neg_zero = read_scalar_as_string(&frame, &neg_zero_var, 0)?;
        assert_eq!(neg_zero, "-0");

        let inf_var = variable("f32", VariableType::Float32, 8, 1);
        let inf = read_scalar_as_string(&frame, &inf_var, 0)?;
        assert_eq!(inf, "inf");

        let neg_inf_var = variable("f32", VariableType::Float32, 12, 1);
        let neg_inf = read_scalar_as_string(&frame, &neg_inf_var, 0)?;
        assert_eq!(neg_inf, "-inf");

        Ok(())
    }
}