use anyhow::Result;
use clap::Subcommand;
use iracing_irsdk::{DiskSubHeader, Header, IbtHeader, VariableBuffer, VariableHeader};
use std::{fs::File, io::Read, path::PathBuf};
use type_layout::TypeLayout;

use crate::writer::{DocumentFormat, DocumentWriter, OutputTarget};

mod layout;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Gets headers from a provided IBT
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Gets headers from a live iRacing connection.
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Inspect physical IBT regions and the runtime telemetry frame layout.
    Layout {
        /// The path of the IBT.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// Output encoding; none prints a human-readable inspection.
        #[arg(long, default_value = "none", value_enum)]
        format: DocumentFormat,
    },
    /// Prints the type information for the header data structures.
    Type,
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Ibt {
            path,
            output,
            format,
        } => {
            // Open the file
            let file = File::open(path)?;
            let mut handle = file.take(size_of::<IbtHeader>() as u64);

            // Read the header
            let header = IbtHeader::try_from_reader(&mut handle)?;

            // Write output
            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&header)?;
            writer.finalize()
        }
        #[cfg(windows)]
        Command::Live { output, format } => {
            use crate::utils::get_connection;

            // Open the connection
            let connection = get_connection()?;
            // Read the header
            let header = connection.header_snapshot()?;

            // Write output
            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&header)?;
            writer.finalize()
        }
        Command::Layout {
            path,
            output,
            format,
        } => {
            use iracing_sdk::{TelemetryLayout, ibt::IbtReader, provider::VariableHeadersProvider};

            let reader = IbtReader::open(path)?;
            let telemetry = TelemetryLayout::try_from_headers(
                &reader.variable_headers()?,
                reader.frame_size(),
            )?;
            let inspection = layout::Inspection::new(reader.layout(), &telemetry);
            let mut writer = DocumentWriter::from_parts(output, format)?;
            match format {
                DocumentFormat::None => writer.write(&inspection.text())?,
                _ => writer.write(&inspection)?,
            }
            writer.finalize()
        }
        Command::Type => {
            let mut writer =
                DocumentWriter::from_parts(OutputTarget::Stdout, DocumentFormat::None)?;

            for layout in [
                VariableBuffer::type_layout(),
                Header::type_layout(),
                DiskSubHeader::type_layout(),
                VariableHeader::type_layout(),
            ] {
                writer.write(&layout.to_string())?;
            }

            writer.finalize()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iracing_sdk::test_utils::require_smallest_ibt_fixture;

    #[test]
    fn layout_command_writes_text_json_and_yaml() -> Result<()> {
        let path = require_smallest_ibt_fixture()?;
        let directory = tempfile::tempdir()?;
        for format in [
            DocumentFormat::None,
            DocumentFormat::Json,
            DocumentFormat::JsonPretty,
            DocumentFormat::Yaml,
        ] {
            let output = directory.path().join(format.to_string());
            handle_command(Command::Layout {
                path: path.clone(),
                output: OutputTarget::File(output.clone()),
                format,
            })?;
            let text = std::fs::read_to_string(output)?;
            if format == DocumentFormat::None {
                assert!(text.contains("IBT layout (source byte offsets)"));
                assert!(text.contains("Telemetry frame layout (frame-relative byte offsets)"));
                assert!(text.contains("offset  end  size  type  count  name"));
            } else {
                let value: serde_json::Value = match format {
                    DocumentFormat::Yaml => serde_yaml_ng::from_str(&text)?,
                    _ => serde_json::from_str(&text)?,
                };
                assert_eq!(value["ibt_layout"]["frame_count"], 12);
                assert_eq!(value["telemetry_frame_layout"]["frame_size"], 48);
                assert_eq!(
                    value["telemetry_frame_layout"]["fields"]
                        .as_array()
                        .unwrap()
                        .len(),
                    8
                );
            }
        }
        let args = crate::Args::try_parse_from([
            "iracing-sdk",
            "headers",
            "layout",
            "--path",
            path.to_str().unwrap(),
        ])?;
        assert!(matches!(
            args.commands,
            crate::Commands::Headers {
                command: Command::Layout {
                    format: DocumentFormat::None,
                    ..
                }
            }
        ));
        Ok(())
    }
}
