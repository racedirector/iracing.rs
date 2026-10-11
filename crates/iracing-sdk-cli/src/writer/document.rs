//! Generic serde document serialization, independent of source acquisition.
//!
//! Commands decide which value to serialize and when it is safe to open output.
//! [`DocumentWriter`] owns formatting and checked final flushing;
//! [`OutputSink`] owns destination opening and byte I/O. Neither owner interprets
//! telemetry geometry, field selection, or simulator readiness.
//!
//! Opening a file destination is immediate and destructive to existing contents.
//! Serialization is not transactional: write failures do not restore the file.
//! Command-level input validation before writer construction is therefore a
//! separate responsibility, not a guarantee supplied by this writer.

use anyhow::Result;
use clap::ValueEnum;
use std::{convert::Infallible, fmt, io::Write, path::PathBuf, str::FromStr};

use super::output_sink::OutputSink;

/// Representation selected for a generic serde document.
///
/// Clap's `ValueEnum` declaration owns operator vocabulary. Formatting behavior
/// belongs to [`DocumentWriter::write`], independent of the serialized value's
/// source or schema.
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
///
/// Parsing identifies a destination without opening it. [`OutputSink`] performs
/// opening when a writer is constructed; [`fmt::Display`] provides a diagnostic
/// label rather than a reversible representation of the parsed input.
#[derive(Debug, Clone)]
pub(crate) enum OutputTarget {
    Stdout,
    File(PathBuf),
}

impl FromStr for OutputTarget {
    type Err = Infallible;

    /// Interpret exactly `-` as stdout and every other string as a file path.
    ///
    /// Performs no I/O, path existence check, or directory creation. Parsing is
    /// infallible; an unusable destination fails later when the sink is opened.
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

/// Stateful formatter for caller-supplied serde values.
///
/// Construction consumes the destination and returns a ready writer. Repeated
/// writes append separately formatted values to that sink; this type does not
/// enforce a one-document limit or wrap them into an aggregate. Callers own the
/// output's semantic shape. [`Self::finalize`] checks the final flush.
pub(crate) struct DocumentWriter<S> {
    state: S,
}

/// Open output destination and chosen document representation.
pub struct Ready {
    writer: OutputSink,
    format: DocumentFormat,
}

impl DocumentWriter<()> {
    /// Open the destination immediately and retain the selected representation.
    ///
    /// File output is created or truncated before any value is serialized,
    /// including for the raw representation. This method does not validate the
    /// caller's data, create parent directories, or stage an atomic replacement.
    ///
    /// # Errors
    ///
    /// Propagates destination-opening errors from [`OutputSink`]. Source and
    /// value validation should happen before this call when a command promises
    /// to preserve existing output on input failure.
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
    /// Serialize one caller-supplied value to the already-open destination.
    ///
    /// Compact and pretty JSON use serde's corresponding writers, followed by
    /// one LF byte after successful serialization. YAML delegates its framing
    /// to `serde_yaml_ng::to_writer`, without adding another newline here.
    ///
    /// The raw (`None`) representation first converts to a JSON value: strings
    /// are written without JSON quotes or escaping, while other values use JSON
    /// value display. Both receive one trailing LF; existing newlines in a raw
    /// string are retained. This mode still invokes serde serialization.
    ///
    /// Writes do not explicitly flush. JSON/YAML serialization can write a
    /// prefix before failing; raw conversion happens before its write, whose
    /// I/O can still fail partway through. No mode rolls back bytes already
    /// written, and adding a JSON newline can fail after the value was emitted.
    ///
    /// # Errors
    ///
    /// Propagates serialization and output I/O errors. Successful serialization
    /// does not certify source validity or establish durable file storage.
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

    /// Consume the writer and flush its destination, reporting any flush error.
    ///
    /// This is an I/O flush, not a filesystem durability operation such as
    /// `sync_all`. Dropping without this call does not report buffered drop-time
    /// flush failures to the command.
    pub fn finalize(mut self) -> Result<()> {
        self.state.writer.flush()?;
        Ok(())
    }
}
