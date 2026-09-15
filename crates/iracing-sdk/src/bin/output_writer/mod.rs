use anyhow::Result;
use clap::ValueEnum;
use std::{
    convert::Infallible,
    fmt,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputEncoding {
    Json,
    JsonPretty,
    Yaml,
}

#[derive(Debug, Clone)]
pub(super) enum OutputTarget {
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

pub(super) fn write_to_output<T>(
    value: &T,
    target: &OutputTarget,
    encoding: OutputEncoding,
) -> Result<()>
where
    T: ?Sized + serde::Serialize,
{
    match target {
        OutputTarget::Stdout => {
            let stdout = std::io::stdout();
            write_to_writer(value, stdout.lock(), encoding)
        }
        OutputTarget::File(path) => {
            let writer = BufWriter::new(File::create(path)?);
            write_to_writer(value, writer, encoding)
        }
    }
}

fn write_to_writer<T, W>(value: &T, mut writer: W, encoding: OutputEncoding) -> Result<()>
where
    T: ?Sized + serde::Serialize,
    W: Write,
{
    match encoding {
        OutputEncoding::Yaml => serde_yaml_ng::to_writer(&mut writer, value)?,
        OutputEncoding::Json => serde_json::to_writer(&mut writer, value)?,
        OutputEncoding::JsonPretty => serde_json::to_writer_pretty(&mut writer, value)?,
    }
    writer.flush()?;
    Ok(())
}
