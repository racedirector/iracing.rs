use anyhow::{Context, Result, anyhow};
use csv::Writer;
use iracing_sdk::{TelemetryValue, TelemetryValueProvider, VariableInfo};
use std::{fs::File, path::PathBuf};

pub(super) struct CsvTelemetryWriter {
    writer: Writer<File>,
    variables: Vec<VariableInfo>,
    column_count: usize,
}

#[derive(Default)]
pub(super) struct CsvTelemetryWriterBuilder {
    output_path: Option<PathBuf>,
    variables: Option<Vec<VariableInfo>>,
}

impl CsvTelemetryWriter {
    pub(super) fn builder() -> CsvTelemetryWriterBuilder {
        CsvTelemetryWriterBuilder::default()
    }

    pub(super) fn variable_count(&self) -> usize {
        self.variables.len()
    }

    pub(super) fn column_count(&self) -> usize {
        self.column_count
    }

    pub(super) fn write_telemetry(&mut self, provider: &dyn TelemetryValueProvider) -> Result<()> {
        let mut row = Vec::with_capacity(self.column_count);

        for variable in &self.variables {
            let value = provider
                .telemetry_value_from_info(variable)
                .with_context(|| format!("Failed to decode `{}`", variable.name))?;

            Self::append_value(&mut row, value);
        }

        if row.len() != self.column_count {
            return Err(anyhow!(
                "Internal CSV row width mismatch: expected {} columns, found {}",
                self.column_count,
                row.len()
            ));
        }

        self.writer.write_record(&row)?;
        Ok(())
    }

    pub(super) fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
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
                    Self::append_value(row, value);
                }
            }
        }
    }
}

impl CsvTelemetryWriterBuilder {
    pub(super) fn with_output_path(mut self, output_path: impl Into<PathBuf>) -> Self {
        self.output_path = Some(output_path.into());
        self
    }

    pub(super) fn with_variables(mut self, variables: Vec<VariableInfo>) -> Self {
        self.variables = Some(variables);
        self
    }

    pub(super) fn build(self) -> Result<CsvTelemetryWriter> {
        let output_path = self
            .output_path
            .ok_or_else(|| anyhow!("CSV output path is required"))?;
        let variables = self
            .variables
            .ok_or_else(|| anyhow!("Telemetry variables are required"))?;

        let mut headers =
            Vec::with_capacity(variables.iter().map(|variable| variable.count.max(1)).sum());
        for variable in &variables {
            if variable.count <= 1 {
                headers.push(variable.name.clone());
                continue;
            }

            for index in 0..variable.count {
                headers.push(format!("{}[{}]", variable.name, index));
            }
        }

        let column_count = headers.len();
        let mut writer = Writer::from_path(&output_path)
            .with_context(|| format!("Could not create CSV output at {}", output_path.display()))?;
        writer.write_record(&headers)?;

        Ok(CsvTelemetryWriter {
            writer,
            variables,
            column_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::VariableType;
    use std::{
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestTelemetryValueProvider {
        values: HashMap<String, TelemetryValue>,
    }

    impl TelemetryValueProvider for TestTelemetryValueProvider {
        fn telemetry_value_from_info(
            &self,
            info: &VariableInfo,
        ) -> iracing_sdk::Result<TelemetryValue> {
            Ok(self.values[&info.name].clone())
        }
    }

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
    fn append_value_formats_scalars_and_flattens_arrays() {
        let mut row = Vec::new();
        CsvTelemetryWriter::append_value(&mut row, TelemetryValue::Float32(10.0));
        CsvTelemetryWriter::append_value(&mut row, TelemetryValue::Bool(true));
        CsvTelemetryWriter::append_value(
            &mut row,
            TelemetryValue::Array(vec![TelemetryValue::UInt32(2), TelemetryValue::UInt32(3)]),
        );

        assert_eq!(row, vec!["10", "true", "2", "3"]);
    }

    #[test]
    fn builder_requires_variables() {
        let error = CsvTelemetryWriter::builder()
            .with_output_path("unused.csv")
            .build()
            .err()
            .expect("build should fail without variables");

        assert!(error.to_string().contains("Telemetry variables"));
    }

    #[test]
    fn writer_emits_headers_and_telemetry_rows() -> Result<()> {
        let speed = variable("Speed", VariableType::Float32, 0, 1);
        let lap_distances = variable("CarIdxLapDistPct", VariableType::Float32, 4, 3);
        let variables = vec![speed, lap_distances];
        let provider = TestTelemetryValueProvider {
            values: HashMap::from([
                ("Speed".to_string(), TelemetryValue::Float32(42.5)),
                (
                    "CarIdxLapDistPct".to_string(),
                    TelemetryValue::Array(vec![
                        TelemetryValue::Float32(0.1),
                        TelemetryValue::Float32(0.2),
                        TelemetryValue::Float32(0.3),
                    ]),
                ),
            ]),
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let output_path =
            std::env::temp_dir().join(format!("telemetry-csv-{}-{unique}.csv", std::process::id()));

        {
            let mut writer = CsvTelemetryWriter::builder()
                .with_output_path(&output_path)
                .with_variables(variables)
                .build()?;
            writer.write_telemetry(&provider)?;
            writer.flush()?;
        }

        let output = std::fs::read_to_string(&output_path)?;
        std::fs::remove_file(&output_path)?;

        assert_eq!(
            output,
            "Speed,CarIdxLapDistPct[0],CarIdxLapDistPct[1],CarIdxLapDistPct[2]\n42.5,0.1,0.2,0.3\n"
        );
        Ok(())
    }
}
