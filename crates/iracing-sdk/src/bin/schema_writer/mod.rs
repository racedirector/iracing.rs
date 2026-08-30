use anyhow::Result;
use clap::ValueEnum;
use std::{fs::File, io::BufWriter, path::PathBuf};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SchemaOutputEncoding {
    Json,
    JsonPretty,
    Yaml,
}

pub(super) fn write_to_output<T>(
    value: &T,
    output_path: &PathBuf,
    format: SchemaOutputEncoding,
) -> Result<()>
where
    T: ?Sized + serde::Serialize,
{
    let output_file = File::create(output_path)?;
    let writer = BufWriter::new(output_file);

    match format {
        SchemaOutputEncoding::Yaml => {
            serde_yaml_ng::to_writer(writer, &value)?;
        }
        SchemaOutputEncoding::Json => {
            serde_json::to_writer(writer, &value)?;
        }
        SchemaOutputEncoding::JsonPretty => {
            serde_json::to_writer_pretty(writer, &value)?;
        }
    }

    Ok(())
}
