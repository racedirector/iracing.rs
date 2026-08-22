use anyhow::{Context, Result};
use iracing_sdk::{DynamicFrame, TelemetryValue, TelemetryValueProvider, VariableInfo};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;

pub(super) struct JsonTelemetryWriter {
    writer: BufWriter<File>,
}

#[derive(Debug)]
pub(super) struct DynamicFrameSnapshot<'a> {
    telemetry: Vec<(&'a str, TelemetryValue)>,
}

impl JsonTelemetryWriter {
    pub(super) fn from_path(output_path: impl AsRef<Path>) -> Result<Self> {
        let output_path = output_path.as_ref();
        let file = File::create(output_path).with_context(|| {
            format!("Could not create JSONL output at {}", output_path.display())
        })?;

        Ok(Self {
            writer: BufWriter::with_capacity(OUTPUT_BUFFER_CAPACITY, file),
        })
    }

    pub(super) fn write_snapshot(&mut self, snapshot: &DynamicFrameSnapshot<'_>) -> Result<()> {
        serde_json::to_writer(&mut self.writer, snapshot)
            .context("Could not serialize JSONL snapshot")?;
        self.writer
            .write_all(b"\n")
            .context("Could not write JSONL snapshot")?;
        Ok(())
    }

    #[cfg(test)]
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

impl<'a> DynamicFrameSnapshot<'a> {
    pub(super) fn from_frame(frame: &DynamicFrame, variables: &'a [VariableInfo]) -> Result<Self> {
        Self::from_provider(frame, variables)
    }

    fn from_provider(
        provider: &dyn TelemetryValueProvider,
        variables: &'a [VariableInfo],
    ) -> Result<Self> {
        let mut telemetry = Vec::with_capacity(variables.len());

        for variable in variables {
            let value = provider
                .telemetry_value(variable)
                .with_context(|| format!("Failed to decode `{}`", variable.name))?;
            telemetry.push((variable.name.as_str(), value));
        }

        Ok(Self { telemetry })
    }
}

impl Serialize for DynamicFrameSnapshot<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        OrderedTelemetry(&self.telemetry).serialize(serializer)
    }
}

struct OrderedTelemetry<'a>(&'a [(&'a str, TelemetryValue)]);

impl Serialize for OrderedTelemetry<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut telemetry = serializer.serialize_map(Some(self.0.len()))?;
        for (name, value) in self.0 {
            telemetry.serialize_entry(name, &SerializableTelemetryValue(value))?;
        }
        telemetry.end()
    }
}

struct SerializableTelemetryValue<'a>(&'a TelemetryValue);

impl Serialize for SerializableTelemetryValue<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            TelemetryValue::Char(value) => serializer.serialize_char(char::from(*value)),
            TelemetryValue::Int8(value) => serializer.serialize_i8(*value),
            TelemetryValue::UInt8(value) => serializer.serialize_u8(*value),
            TelemetryValue::Int16(value) => serializer.serialize_i16(*value),
            TelemetryValue::UInt16(value) => serializer.serialize_u16(*value),
            TelemetryValue::Int32(value) => serializer.serialize_i32(*value),
            TelemetryValue::UInt32(value) => serializer.serialize_u32(*value),
            TelemetryValue::Float32(value) => serializer.serialize_f32(*value),
            TelemetryValue::Float64(value) => serializer.serialize_f64(*value),
            TelemetryValue::Bool(value) => serializer.serialize_bool(*value),
            TelemetryValue::BitField(value) => serializer.serialize_u32(value.value()),
            TelemetryValue::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&SerializableTelemetryValue(value))?;
                }
                sequence.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::{
        AdapterValidation, BitField, FrameAdapter, FramePacket, IRacingSDKError, VariableSchema,
        VariableType,
    };
    use serde::ser::Error as _;
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestTelemetryValueProvider {
        values: HashMap<String, TelemetryValue>,
        failing_variable: Option<String>,
    }

    impl TelemetryValueProvider for TestTelemetryValueProvider {
        fn telemetry_value(&self, info: &VariableInfo) -> iracing_sdk::Result<TelemetryValue> {
            if self.failing_variable.as_deref() == Some(info.name.as_str()) {
                return Err(IRacingSDKError::parse_error(
                    "test provider",
                    "intentional failure",
                ));
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
            (TelemetryValue::Float32(f32::NAN), serde_json::Value::Null),
            (
                TelemetryValue::Float64(f64::INFINITY),
                serde_json::Value::Null,
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(
                serde_json::to_value(SerializableTelemetryValue(&value)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn float32_values_use_the_shortest_round_trippable_representation() -> Result<()> {
        let value = TelemetryValue::Float32(0.1);

        assert_eq!(
            serde_json::to_string(&SerializableTelemetryValue(&value))?,
            "0.1"
        );
        Ok(())
    }

    #[test]
    fn dynamic_snapshot_preserves_variable_order() -> Result<()> {
        let variables = vec![variable("Speed"), variable("Gear")];
        let provider = TestTelemetryValueProvider {
            values: HashMap::from([
                ("Speed".to_string(), TelemetryValue::Float32(42.5)),
                ("Gear".to_string(), TelemetryValue::Int32(3)),
            ]),
            failing_variable: None,
        };

        let snapshot = DynamicFrameSnapshot::from_provider(&provider, &variables)?;
        let output_path = output_path("ordered-dynamic-snapshot");
        {
            let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
            writer.write_snapshot(&snapshot)?;
            writer.flush()?;
        }

        let output = std::fs::read_to_string(&output_path)?;
        std::fs::remove_file(&output_path)?;
        assert_eq!(output, "{\"Speed\":42.5,\"Gear\":3}\n");
        Ok(())
    }

    #[test]
    fn dynamic_snapshot_decodes_a_dynamic_frame() -> Result<()> {
        let value = variable("Value");
        let variables = vec![value.clone()];
        let schema = VariableSchema {
            variables: HashMap::from([(value.name.clone(), value)]),
            frame_size: 4,
        };
        let packet = FramePacket::new(42i32.to_le_bytes().to_vec(), 1, 0, Arc::new(schema));
        let frame = DynamicFrame::adapt(&packet, &AdapterValidation::new(Vec::new()));

        let snapshot = DynamicFrameSnapshot::from_frame(&frame, &variables)?;

        assert_eq!(
            serde_json::to_value(snapshot)?,
            serde_json::json!({ "Value": 42 })
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

        let error = DynamicFrameSnapshot::from_provider(&provider, &variables)
            .expect_err("snapshot construction should fail");
        assert!(error.to_string().contains("Failed to decode `Broken`"));
    }
}
