use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
#[cfg(test)]
use iracing_irsdk::VariableHeader;
use iracing_sdk::{FieldLayout, TelemetryValue, TelemetryValueProvider};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use std::{fmt, io::Write};

use crate::writer::{OutputTarget, output_sink::OutputSink};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordStreamFormat {
    Jsonl,
    Csv,
}

impl fmt::Display for RecordStreamFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jsonl => f.write_str("json-lines"),
            Self::Csv => f.write_str("csv"),
        }
    }
}

enum RecordEncoder {
    Jsonl {
        writer: OutputSink,
        variables: Vec<FieldLayout>,
    },
    Csv {
        writer: Box<csv::Writer<OutputSink>>,
        variables: Vec<FieldLayout>,
        column_count: usize,
    },
}

pub(crate) struct RecordStreamWriter<S> {
    state: S,
}

pub struct NeedsPreparation {
    sink: OutputSink,
    format: RecordStreamFormat,
    variables: Vec<FieldLayout>,
}

pub struct Ready {
    encoder: RecordEncoder,
}

impl RecordStreamWriter<()> {
    #[cfg(test)]
    pub fn from_parts(
        target: OutputTarget,
        format: RecordStreamFormat,
        variables: &[VariableHeader],
    ) -> Result<RecordStreamWriter<NeedsPreparation>> {
        let variables = test_fields(variables)?;
        Self::from_variables(target, format, variables)
    }

    pub fn from_variables(
        target: OutputTarget,
        format: RecordStreamFormat,
        variables: Vec<FieldLayout>,
    ) -> Result<RecordStreamWriter<NeedsPreparation>> {
        Ok(RecordStreamWriter {
            state: NeedsPreparation {
                sink: OutputSink::try_from(target)?,
                format,
                variables,
            },
        })
    }
}

impl RecordStreamWriter<NeedsPreparation> {
    pub fn prepare(self) -> Result<RecordStreamWriter<Ready>> {
        let NeedsPreparation {
            sink: writer,
            format,
            variables,
        } = self.state;

        let encoder = match format {
            RecordStreamFormat::Jsonl => RecordEncoder::Jsonl { writer, variables },
            RecordStreamFormat::Csv => {
                let mut writer = csv::WriterBuilder::new()
                    .has_headers(false)
                    .from_writer(writer);
                let mut headers = Vec::with_capacity(
                    variables
                        .iter()
                        .map(|variable| variable.count().max(1))
                        .sum(),
                );
                for variable in &variables {
                    if variable.count() <= 1 {
                        headers.push(variable.name().to_owned());
                        continue;
                    }
                    for index in 0..variable.count() {
                        headers.push(format!("{}[{}]", variable.name(), index));
                    }
                }
                writer.write_record(&headers)?;
                RecordEncoder::Csv {
                    writer: Box::new(writer),
                    variables,
                    column_count: headers.len(),
                }
            }
        };
        Ok(RecordStreamWriter {
            state: Ready { encoder },
        })
    }
}

impl RecordStreamWriter<Ready> {
    pub fn write(&mut self, telemetry: &dyn TelemetryValueProvider) -> Result<()> {
        match &mut self.state.encoder {
            RecordEncoder::Jsonl { writer, variables } => {
                let snapshot = TelemetrySnapshot::from_provider(telemetry, variables)?;
                serde_json::to_writer(&mut *writer, &snapshot)?;
                writer.write_all(b"\n")?;
            }
            RecordEncoder::Csv {
                writer,
                variables,
                column_count,
            } => {
                let values = decode_values(telemetry, variables)?;
                let mut row = Vec::with_capacity(*column_count);
                for (variable, value) in variables.iter().zip(&values) {
                    let start = row.len();
                    match value {
                        TelemetryValue::Array(values) => {
                            for value in values {
                                ensure!(
                                    !matches!(value, TelemetryValue::Array(_)),
                                    "Nested array in `{}`",
                                    variable.name()
                                );
                                row.push(Some(SerializableValue(value)));
                            }
                            // Keep the named column for an empty variable.
                            if values.is_empty() && variable.count() == 0 {
                                row.push(None);
                            }
                        }
                        value => row.push(Some(SerializableValue(value))),
                    }
                    ensure!(
                        row.len() - start == variable.count().max(1),
                        "CSV column count mismatch for `{}`",
                        variable.name()
                    );
                }
                writer.serialize(&row)?;
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        match &mut self.state.encoder {
            RecordEncoder::Jsonl { writer, .. } => writer.flush()?,
            RecordEncoder::Csv { writer, .. } => writer.flush()?,
        }
        Ok(())
    }
}

fn decode_values(
    provider: &dyn TelemetryValueProvider,
    variables: &[FieldLayout],
) -> Result<Vec<TelemetryValue>> {
    variables
        .iter()
        .map(|variable| {
            provider
                .telemetry_value(variable)
                .with_context(|| format!("Failed to decode `{}`", variable.name()))
        })
        .collect()
}

pub(crate) struct TelemetrySnapshot<'a> {
    variables: &'a [FieldLayout],
    values: Vec<TelemetryValue>,
}

impl<'a> TelemetrySnapshot<'a> {
    pub(crate) fn from_provider(
        provider: &dyn TelemetryValueProvider,
        variables: &'a [FieldLayout],
    ) -> Result<Self> {
        Ok(Self {
            variables,
            values: decode_values(provider, variables)?,
        })
    }
}
impl Serialize for TelemetrySnapshot<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.variables.len()))?;
        for (variable, value) in self.variables.iter().zip(&self.values) {
            map.serialize_entry(&variable.name(), &SerializableValue(value))?;
        }
        map.end()
    }
}

// Export plain values rather than TelemetryValue's externally tagged enum representation.
struct SerializableValue<'a>(&'a TelemetryValue);

impl Serialize for SerializableValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            TelemetryValue::Char(value) => serializer.serialize_char(char::from(*value)),
            TelemetryValue::Int32(value) => serializer.serialize_i32(*value),
            TelemetryValue::Float32(value) => serializer.serialize_f32(*value),
            TelemetryValue::Float64(value) => serializer.serialize_f64(*value),
            TelemetryValue::Bool(value) => serializer.serialize_bool(*value),
            TelemetryValue::BitField(value) => serializer.serialize_u32(value.value()),
            TelemetryValue::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&SerializableValue(value))?;
                }
                sequence.end()
            }
        }
    }
}

#[cfg(test)]
fn test_fields(headers: &[VariableHeader]) -> Result<Vec<FieldLayout>> {
    let layout = iracing_sdk::TelemetryLayout::try_from_headers(&headers.to_vec().into(), 8587)?;
    Ok(layout.fields().map(|(_, field)| field.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_irsdk::VariableType;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestOutput(PathBuf);

    impl TestOutput {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(
                std::env::temp_dir().join(format!("record-stream-{}-{unique}", std::process::id())),
            )
        }
    }

    impl Drop for TestOutput {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    struct Provider {
        fail: bool,
        lap_count: usize,
    }

    impl TelemetryValueProvider for Provider {
        fn telemetry_value(&self, info: &FieldLayout) -> iracing_sdk::Result<TelemetryValue> {
            match info.name() {
                "Speed" => Ok(TelemetryValue::Float32(42.5)),
                "CarIdxLap" if !self.fail => {
                    Ok(TelemetryValue::Array(vec![
                        TelemetryValue::Int32(2);
                        self.lap_count
                    ]))
                }
                _ => Err(iracing_sdk::IRacingSDKError::parse_error(
                    "test",
                    "decode failed",
                )),
            }
        }
    }

    fn headers() -> Vec<VariableHeader> {
        // Metadata from docs/reference/live-variable-schema.yml; no frame bytes are synthesized.
        vec![
            VariableHeader::new(
                VariableType::Float,
                7125,
                1,
                false,
                "Speed",
                "GPS vehicle speed",
                "m/s",
            )
            .unwrap(),
            VariableHeader::new(
                VariableType::Integer,
                212,
                72,
                false,
                "CarIdxLap",
                "Laps started by car index",
                "",
            )
            .unwrap(),
        ]
    }

    #[test]
    fn snapshot_writes_plain_values_in_every_document_format() -> Result<()> {
        use crate::writer::{DocumentFormat, DocumentWriter};

        let variables = test_fields(&headers())?;
        let snapshot = TelemetrySnapshot::from_provider(
            &Provider {
                fail: false,
                lap_count: 72,
            },
            &variables,
        )?;
        for format in [
            DocumentFormat::Json,
            DocumentFormat::JsonPretty,
            DocumentFormat::Yaml,
            DocumentFormat::None,
        ] {
            let output = TestOutput::new();
            let mut writer =
                DocumentWriter::from_parts(OutputTarget::File(output.0.clone()), format)?;
            writer.write(&snapshot)?;
            writer.finalize()?;
            let text = fs::read_to_string(&output.0)?;
            let value: serde_json::Value = match format {
                DocumentFormat::Yaml => serde_yaml_ng::from_str(&text)?,
                _ => serde_json::from_str(&text)?,
            };
            assert_eq!(
                value,
                serde_json::json!({"Speed": 42.5, "CarIdxLap": vec![2; 72]})
            );
        }
        assert!(
            TelemetrySnapshot::from_provider(
                &Provider {
                    fail: true,
                    lap_count: 72
                },
                &variables,
            )
            .err()
            .unwrap()
            .to_string()
            .contains("CarIdxLap")
        );
        Ok(())
    }
    #[test]
    fn writes_multiple_records_in_both_formats() -> Result<()> {
        for format in [RecordStreamFormat::Jsonl, RecordStreamFormat::Csv] {
            let output = TestOutput::new();
            let mut writer = RecordStreamWriter::from_parts(
                OutputTarget::File(output.0.clone()),
                format,
                &headers(),
            )?
            .prepare()?;
            for _ in 0..2 {
                writer.write(&Provider {
                    fail: false,
                    lap_count: 72,
                })?;
            }
            writer.finalize()?;
            let text = fs::read_to_string(&output.0)?;
            match format {
                RecordStreamFormat::Jsonl => {
                    assert_eq!(text.lines().count(), 2);
                    assert!(text.ends_with('\n'));
                    for line in text.lines() {
                        assert!(line.starts_with("{\"Speed\":42.5,\"CarIdxLap\":"));
                        let value: serde_json::Value = serde_json::from_str(line)?;
                        assert_eq!(value["CarIdxLap"], serde_json::json!(vec![2; 72]));
                    }
                }
                RecordStreamFormat::Csv => {
                    let mut reader = csv::Reader::from_reader(text.as_bytes());
                    let header = reader.headers()?;
                    assert_eq!(header.len(), 73);
                    assert_eq!(&header[0], "Speed");
                    assert_eq!(&header[72], "CarIdxLap[71]");
                    let rows = reader
                        .records()
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    assert_eq!(rows.len(), 2);
                    for row in rows {
                        assert_eq!(&row[0], "42.5");
                        assert!(row.iter().skip(1).all(|value| value == "2"));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn failed_decode_does_not_write_a_partial_record() -> Result<()> {
        for format in [RecordStreamFormat::Jsonl, RecordStreamFormat::Csv] {
            let output = TestOutput::new();
            let mut writer = RecordStreamWriter::from_parts(
                OutputTarget::File(output.0.clone()),
                format,
                &headers(),
            )?
            .prepare()?;
            let error = writer
                .write(&Provider {
                    fail: true,
                    lap_count: 72,
                })
                .unwrap_err();
            assert!(error.to_string().contains("CarIdxLap"));
            writer.write(&Provider {
                fail: false,
                lap_count: 72,
            })?;
            writer.finalize()?;
            let text = fs::read_to_string(&output.0)?;
            assert_eq!(
                text.lines().count(),
                if format == RecordStreamFormat::Csv {
                    2
                } else {
                    1
                }
            );
        }
        Ok(())
    }

    #[test]
    fn csv_rejects_wrong_array_width_before_writing() -> Result<()> {
        let output = TestOutput::new();
        let mut writer = RecordStreamWriter::from_parts(
            OutputTarget::File(output.0.clone()),
            RecordStreamFormat::Csv,
            &headers(),
        )?
        .prepare()?;
        assert!(
            writer
                .write(&Provider {
                    fail: false,
                    lap_count: 71
                })
                .unwrap_err()
                .to_string()
                .contains("CarIdxLap")
        );
        writer.finalize()?;
        assert_eq!(fs::read_to_string(&output.0)?.lines().count(), 1);
        Ok(())
    }

    #[test]
    fn serde_encodes_plain_scalars_and_csv_escaping() -> Result<()> {
        let values = [
            TelemetryValue::Char(b','),
            TelemetryValue::Bool(true),
            TelemetryValue::Int32(-2),
            TelemetryValue::Float32(1.5),
            TelemetryValue::Float64(2.5),
            TelemetryValue::BitField(iracing_sdk::BitField::new(3)),
        ];
        let values: Vec<_> = values.iter().map(SerializableValue).collect();
        assert_eq!(serde_json::to_string(&values)?, "[\",\",true,-2,1.5,2.5,3]");
        let mut csv = csv::Writer::from_writer(Vec::new());
        csv.serialize(&values)?;
        assert_eq!(
            String::from_utf8(csv.into_inner()?)?,
            "\",\",true,-2,1.5,2.5,3\n"
        );
        Ok(())
    }
}
