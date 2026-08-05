use anyhow::{Context, Result};
use iracing_sdk::{DynamicFrame, TelemetryValue, TelemetryValueProvider, VariableInfo};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub(super) struct JsonTelemetryWriter {
    writer: BufWriter<File>,
}

#[derive(Debug, Serialize)]
pub(super) struct DynamicFrameSnapshot {
    tick_count: u32,
    telemetry: Map<String, Value>,
}

impl JsonTelemetryWriter {
    pub(super) fn from_path(output_path: impl AsRef<Path>) -> Result<Self> {
        let output_path = output_path.as_ref();
        let file = File::create(output_path).with_context(|| {
            format!("Could not create JSONL output at {}", output_path.display())
        })?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub(super) fn write_record<T>(&mut self, record: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let mut encoded = serde_json::to_vec(record).context("Could not serialize JSONL record")?;
        encoded.push(b'\n');
        self.writer
            .write_all(&encoded)
            .context("Could not write JSONL record")?;
        Ok(())
    }

    pub(super) fn flush(&mut self) -> Result<()> {
        self.writer.flush().context("Could not flush JSONL output")
    }
}

impl DynamicFrameSnapshot {
    pub(super) fn from_frame(frame: &DynamicFrame, variables: &[VariableInfo]) -> Result<Self> {
        Self::from_provider(frame.tick_count(), frame, variables)
    }

    fn from_provider(
        tick_count: u32,
        provider: &dyn TelemetryValueProvider,
        variables: &[VariableInfo],
    ) -> Result<Self> {
        let mut telemetry = Map::with_capacity(variables.len());

        for variable in variables {
            let value = provider
                .telemetry_value(variable)
                .with_context(|| format!("Failed to decode `{}`", variable.name))?;
            telemetry.insert(variable.name.clone(), telemetry_value_to_json(value));
        }

        Ok(Self {
            tick_count,
            telemetry,
        })
    }
}

fn telemetry_value_to_json(value: TelemetryValue) -> Value {
    match value {
        TelemetryValue::Char(value) => Value::String(char::from(value).to_string()),
        TelemetryValue::Int8(value) => Value::from(value),
        TelemetryValue::UInt8(value) => Value::from(value),
        TelemetryValue::Int16(value) => Value::from(value),
        TelemetryValue::UInt16(value) => Value::from(value),
        TelemetryValue::Int32(value) => Value::from(value),
        TelemetryValue::UInt32(value) => Value::from(value),
        TelemetryValue::Float32(value) => Number::from_f64(f64::from(value))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        TelemetryValue::Float64(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        TelemetryValue::Bool(value) => Value::from(value),
        TelemetryValue::BitField(value) => Value::from(value.value()),
        TelemetryValue::Array(values) => {
            Value::Array(values.into_iter().map(telemetry_value_to_json).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::{BitField, IRacingSDKError, VariableType};
    use serde::ser::Error as _;
    use std::{
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestTelemetryValueProvider {
        values: HashMap<String, TelemetryValue>,
        failing_variable: Option<String>,
    }

    impl TelemetryValueProvider for TestTelemetryValueProvider {
        fn telemetry_value(&self, info: &VariableInfo) -> iracing_sdk::Result<TelemetryValue> {
            if self.failing_variable.as_deref() == Some(info.name.as_str()) {
                return Err(IRacingSDKError::Parse {
                    context: "test provider".to_string(),
                    details: "intentional failure".to_string(),
                });
            }

            Ok(self.values[&info.name].clone())
        }
    }

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(S::Error::custom("intentional failure"))
        }
    }

    fn output_path(test_name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{test_name}-{}-{unique}.jsonl", std::process::id()))
    }

    fn variable(name: &str) -> VariableInfo {
        VariableInfo {
            name: name.to_string(),
            data_type: VariableType::Int32,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn writer_emits_one_compact_json_value_per_line() -> Result<()> {
        #[derive(Serialize)]
        struct DerivedFrame<'a> {
            speed: f32,
            labels: &'a [&'a str],
        }

        let output_path = output_path("generic-jsonl-writer");
        {
            let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
            writer.write_record(&DerivedFrame {
                speed: 42.5,
                labels: &["player", "leader"],
            })?;
            writer.write_record(&DerivedFrame {
                speed: 43.0,
                labels: &[],
            })?;
            writer.flush()?;
        }

        let output = std::fs::read_to_string(&output_path)?;
        std::fs::remove_file(&output_path)?;
        assert_eq!(
            output,
            "{\"speed\":42.5,\"labels\":[\"player\",\"leader\"]}\n{\"speed\":43.0,\"labels\":[]}\n"
        );
        Ok(())
    }

    #[test]
    fn writer_contextualizes_output_creation_errors() {
        let error = JsonTelemetryWriter::from_path(std::env::temp_dir())
            .err()
            .expect("creating a file at a directory path should fail");

        assert!(error.to_string().contains("Could not create JSONL output"));
    }

    #[test]
    fn serialization_failure_does_not_emit_a_partial_record() -> Result<()> {
        let output_path = output_path("failed-jsonl-record");
        {
            let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
            writer.write_record(&serde_json::json!({ "valid": true }))?;
            let error = writer
                .write_record(&FailingSerialize)
                .expect_err("serialization should fail");
            assert!(error.to_string().contains("serialize JSONL record"));
            writer.flush()?;
        }

        let output = std::fs::read_to_string(&output_path)?;
        std::fs::remove_file(&output_path)?;
        assert_eq!(output, "{\"valid\":true}\n");
        Ok(())
    }

    #[test]
    fn telemetry_values_use_native_json_representations() {
        let cases = [
            (TelemetryValue::Char(b'R'), serde_json::json!("R")),
            (TelemetryValue::Int8(-1), serde_json::json!(-1)),
            (TelemetryValue::UInt8(2), serde_json::json!(2)),
            (TelemetryValue::Int16(-3), serde_json::json!(-3)),
            (TelemetryValue::UInt16(4), serde_json::json!(4)),
            (TelemetryValue::Int32(-5), serde_json::json!(-5)),
            (TelemetryValue::UInt32(6), serde_json::json!(6)),
            (TelemetryValue::Float32(7.5), serde_json::json!(7.5)),
            (TelemetryValue::Float64(8.5), serde_json::json!(8.5)),
            (TelemetryValue::Bool(true), serde_json::json!(true)),
            (
                TelemetryValue::BitField(BitField::new(9)),
                serde_json::json!(9),
            ),
            (
                TelemetryValue::Array(vec![TelemetryValue::Int32(10), TelemetryValue::Int32(11)]),
                serde_json::json!([10, 11]),
            ),
            (TelemetryValue::Float32(f32::NAN), Value::Null),
            (TelemetryValue::Float64(f64::INFINITY), Value::Null),
        ];

        for (value, expected) in cases {
            assert_eq!(telemetry_value_to_json(value), expected);
        }
    }

    #[test]
    fn dynamic_snapshot_wraps_tick_and_telemetry() -> Result<()> {
        let variables = vec![variable("Speed"), variable("Gear")];
        let provider = TestTelemetryValueProvider {
            values: HashMap::from([
                ("Speed".to_string(), TelemetryValue::Float32(42.5)),
                ("Gear".to_string(), TelemetryValue::Int32(3)),
            ]),
            failing_variable: None,
        };

        let snapshot = DynamicFrameSnapshot::from_provider(123, &provider, &variables)?;
        assert_eq!(
            serde_json::to_value(snapshot)?,
            serde_json::json!({
                "tick_count": 123,
                "telemetry": { "Speed": 42.5, "Gear": 3 }
            })
        );
        Ok(())
    }

    #[test]
    fn dynamic_snapshot_contextualizes_decode_failures() {
        let variables = vec![variable("Broken")];
        let provider = TestTelemetryValueProvider {
            values: HashMap::new(),
            failing_variable: Some("Broken".to_string()),
        };

        let error = DynamicFrameSnapshot::from_provider(0, &provider, &variables)
            .expect_err("snapshot construction should fail");
        assert!(error.to_string().contains("Failed to decode `Broken`"));
    }
}
