//! Destination opening and byte I/O shared by CLI writers.
//!
//! This boundary contains no serialization or telemetry policy. Conversion from
//! [`OutputTarget`] opens the destination immediately; writer construction, not
//! destination parsing, is the file creation/truncation boundary.

use anyhow::{Error as AnyhowError, Result};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use crate::writer::OutputTarget;

/// Byte sink forwarding writes and flushes to stdout or a buffered file.
///
/// File buffering may defer I/O errors until a later write or explicit flush.
/// The sink does not stage atomic replacements, roll back partial output, or
/// interpret the bytes. Checked writer finalization owns explicit flushing.
pub(crate) enum OutputSink {
    Stdout(std::io::Stdout),
    File(BufWriter<File>),
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Stdout(writer) => writer.write(buf),
            Self::File(writer) => writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Stdout(writer) => writer.flush(),
            Self::File(writer) => writer.flush(),
        }
    }
}

impl TryFrom<OutputTarget> for OutputSink {
    type Error = AnyhowError;

    /// Use process stdout or create/truncate a file and retain a `BufWriter`.
    ///
    /// `File::create` owns filesystem opening semantics. Parent directories are
    /// not created, existing contents are not retained, and no file bytes are
    /// serialized by this conversion.
    ///
    /// # Errors
    ///
    /// Propagates file-opening errors. Later write/flush errors are returned by
    /// the corresponding [`Write`] operations.
    fn try_from(target: OutputTarget) -> Result<Self> {
        Ok(match target {
            OutputTarget::Stdout => OutputSink::Stdout(std::io::stdout()),
            OutputTarget::File(path) => OutputSink::File(BufWriter::new(File::create(path)?)),
        })
    }
}
