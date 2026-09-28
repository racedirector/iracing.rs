use std::{convert::Infallible, fmt, path::PathBuf, str::FromStr};

use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputEncoding {
    Json,
    JsonPretty,
    Yaml,
}

impl fmt::Display for OutputEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => f.write_str("json"),
            Self::JsonPretty => f.write_str("json-pretty"),
            Self::Yaml => f.write_str("yaml"),
        }
    }
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
