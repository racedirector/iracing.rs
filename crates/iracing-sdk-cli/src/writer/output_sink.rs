use anyhow::{Error as AnyhowError, Result};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use crate::writer::OutputTarget;

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

    fn try_from(target: OutputTarget) -> Result<Self> {
        Ok(match target {
            OutputTarget::Stdout => OutputSink::Stdout(std::io::stdout()),
            OutputTarget::File(path) => OutputSink::File(BufWriter::new(File::create(path)?)),
        })
    }
}
