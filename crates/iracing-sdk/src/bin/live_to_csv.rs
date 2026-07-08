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
use iracing_sdk::{BitField, FramePacket};
#[cfg(windows)]
use iracing_sdk::{LiveProvider, Provider, WindowsConnection};
#[cfg(any(windows, test))]
use iracing_sdk::{VarData, VariableInfo, VariableType};
use std::fs::File;
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run().await
}

#[cfg(windows)]
async fn run() -> Result<()> {
    let Args { output_path } = Args::parse();

    tracing::info!("Opening iRacing connection");
    let connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;
    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    // Create an instance of sorted variables available from the connection
    let mut variables = connection.get_variables();
    if variables.is_empty() {
        return Err(anyhow!(
            "No telemetry variables were available from the live connection"
        ));
    }

    // Sort the variables by offset
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut writer = CsvWriter::builder()
        .with_path(output_path)
        .with_variables(&variables)
        .build()?;

    let mut provider = LiveProvider::with_connection(connection)?;
    while let Some(packet) = provider.next_frame().await? {
        writer.write_packet(&packet)?;
    }

    // Clean up
    tracing::info!("Finished live CSV export");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_csv is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live_to_csv is only supported on Windows"))
}

struct CsvWriterBuilder<K> {
    path: Option<PathBuf>,
    variables: Option<Vec<VariableInfo>>,
    _state: std::marker::PhantomData<K>,
}

pub struct Unset;
pub struct Set;

impl Default for CsvWriterBuilder<Unset> {
    fn default() -> Self {
        Self {
            path: None,
            variables: None,
            _state: std::marker::PhantomData,
        }
    }
}

impl CsvWriterBuilder<Unset> {
    pub fn with_path(self, path: PathBuf) -> CsvWriterBuilder<Set> {
        CsvWriterBuilder {
            path: Some(path),
            variables: self.variables,
            _state: std::marker::PhantomData,
        }
    }
}

impl CsvWriterBuilder<Set> {
    pub fn with_variables(mut self, variables: &[VariableInfo]) -> Self {
        self.variables = Some(variables.to_vec());
        self
    }

    pub fn build(self) -> Result<CsvWriter> {
        let path = self.path.unwrap();
        let variables = self.variables.unwrap_or_default();

        let mut columns: Vec<CsvColumn> = Vec::with_capacity(expanded_column_count(&variables));

        for variable in variables {
            if variable.count <= 1 {
                columns.push(CsvColumn {
                    name: variable.name.clone(),
                    source: CsvColumnSource::Variable {
                        variable_name: variable.name.clone(),
                        array_index: None,
                    },
                })
            } else {
                for index in 0..variable.count {
                    columns.push(CsvColumn {
                        name: format!("{}[{}]", variable.name, index),
                        source: CsvColumnSource::Variable {
                            variable_name: variable.name.clone(),
                            array_index: Some(index),
                        },
                    })
                }
            }
        }

        let mut writer = Writer::from_path(path)?;

        // Write the header
        writer.write_record(columns.iter().map(|column| column.name.as_str()))?;

        Ok(CsvWriter { writer, columns })
    }
}

enum CsvColumnSource {
    Variable {
        variable_name: String,
        array_index: Option<usize>,
    },
}

struct CsvColumn {
    name: String,
    source: CsvColumnSource,
}

struct CsvWriter {
    writer: Writer<File>,
    columns: Vec<CsvColumn>,
}

impl CsvWriter {
    pub fn builder() -> CsvWriterBuilder<Unset> {
        CsvWriterBuilder::default()
    }

    pub fn write_packet(&mut self, packet: &FramePacket) -> Result<()> {
        let mut row = Vec::with_capacity(self.columns.len());

        for column in &self.columns {
            let value = match &column.source {
                CsvColumnSource::Variable {
                    variable_name,
                    array_index,
                } => match packet.schema.variables.get(variable_name) {
                    Some(info) => read_variable_column(packet, info, *array_index)?,
                    None => String::new(),
                },
            };

            row.push(value);
        }

        self.writer.write_record(&row)?;

        Ok(())
    }
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

fn read_column_value<T>(
    data: &[u8],
    info: &VariableInfo,
    array_index: Option<usize>,
) -> Result<String>
where
    T: VarData + ToString,
{
    if info.count <= 1 {
        return Ok(T::from_bytes(data, info)?.to_string());
    }

    let index = match array_index {
        Some(index) => index,
        None => return Ok(String::new()),
    };

    let values = Vec::<T>::from_bytes(data, info)?;

    Ok(values
        .get(index)
        .map(ToString::to_string)
        .unwrap_or_default())
}

fn read_variable_column(
    packet: &FramePacket,
    info: &VariableInfo,
    array_index: Option<usize>,
) -> Result<String> {
    match info.data_type {
        VariableType::Char | VariableType::UInt8 => {
            read_column_value::<u8>(&packet.data, info, array_index)
        }
        VariableType::Int8 => read_column_value::<i8>(&packet.data, info, array_index),
        VariableType::Int16 => read_column_value::<i16>(&packet.data, info, array_index),
        VariableType::UInt16 => read_column_value::<u16>(&packet.data, info, array_index),
        VariableType::Int32 => read_column_value::<i32>(&packet.data, info, array_index),
        VariableType::UInt32 => read_column_value::<u32>(&packet.data, info, array_index),
        VariableType::Float32 => read_column_value::<f32>(&packet.data, info, array_index),
        VariableType::Float64 => read_column_value::<f64>(&packet.data, info, array_index),
        VariableType::Bool => read_column_value::<bool>(&packet.data, info, array_index),
        VariableType::BitField => read_column_value::<BitField>(&packet.data, info, array_index),
    }
}
