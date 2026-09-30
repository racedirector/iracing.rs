use anyhow::Result;
use clap::ValueEnum;
use std::{convert::Infallible, fmt, io::Write, path::PathBuf, str::FromStr};

use super::output_sink::OutputSink;

/// Represents possible output encodings for non-sequential data
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentFormat {
    Json,
    JsonPretty,
    Yaml,
    None,
}

impl fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => f.write_str("json"),
            Self::JsonPretty => f.write_str("json-pretty"),
            Self::Yaml => f.write_str("yaml"),
            Self::None => f.write_str("none"),
        }
    }
}

/// Output targets for singular and sequential data.
#[derive(Debug, Clone)]
pub(crate) enum OutputTarget {
    Stdout,
    File(PathBuf),
}

impl FromStr for OutputTarget {
    type Err = Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match value {
            "-" => Self::Stdout,
            _ => Self::File(PathBuf::from(value)),
        })
    }
}

impl fmt::Display for OutputTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => f.write_str("stdout"),
            Self::File(path) => write!(f, "{}", path.display()),
        }
    }
}

pub(crate) struct DocumentWriter<S> {
    state: S,
}

pub struct Ready {
    writer: OutputSink,
    format: DocumentFormat,
}

impl DocumentWriter<()> {
    pub fn from_parts(
        target: OutputTarget,
        format: DocumentFormat,
    ) -> Result<DocumentWriter<Ready>> {
        Ok(DocumentWriter {
            state: Ready {
                writer: target.try_into()?,
                format,
            },
        })
    }
}

impl DocumentWriter<Ready> {
    pub fn write<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        let Ready { writer, format } = &mut self.state;

        match format {
            DocumentFormat::Yaml => {
                serde_yaml_ng::to_writer(&mut *writer, value)?;
            }

            DocumentFormat::Json => {
                serde_json::to_writer(&mut *writer, value)?;
                writer.write_all(b"\n")?;
            }

            DocumentFormat::JsonPretty => {
                serde_json::to_writer_pretty(&mut *writer, value)?;
                writer.write_all(b"\n")?;
            }

            DocumentFormat::None => match serde_json::to_value(value)? {
                serde_json::Value::String(text) => {
                    writeln!(writer, "{text}")?;
                }
                value => {
                    writeln!(writer, "{value}")?;
                }
            },
        }

        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        self.state.writer.flush()?;
        Ok(())
    }
}
