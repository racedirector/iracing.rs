use std::{convert::Infallible, fmt, path::PathBuf, str::FromStr};

use clap::ValueEnum;

/// Represents possible output encodings for non-sequential data
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocumentFormat {
    Json,
    JsonPretty,
    Yaml,
}

impl fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => f.write_str("json"),
            Self::JsonPretty => f.write_str("json-pretty"),
            Self::Yaml => f.write_str("yaml"),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecordStreamFormat {
    Jsonl,
    Csv,
}

impl fmt::Display for RecordStreamFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jsonl => f.write_str("json-lines"),
            Self::Csv => f.write_str("csv"),
        }
    }
}

/// Output targets for singular and sequential data.
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
