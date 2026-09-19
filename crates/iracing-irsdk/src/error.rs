//! Errors produced while decoding or validating the SDK wire contract.

use thiserror::Error;

use crate::constants::IRSDK_VER;

/// Result type for wire-contract operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error produced while decoding or validating SDK wire data.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A byte buffer does not match a wire type's required size.
    #[error("Invalid wire size: expected {expected} bytes, received {actual}")]
    WireSize {
        /// Required wire representation size.
        expected: usize,
        /// Number of bytes supplied.
        actual: usize,
    },
    /// The SDK header version does not match the supported version.
    #[error("SDK version mismatch: expected {expected}, found {found}")]
    Version {
        /// Supported SDK version.
        expected: u32,
        /// Version found in the wire data.
        found: u32,
    },
    /// Wire data could not be read or validated.
    #[error("Parse error in {context}: {details}")]
    Parse {
        /// Parsing or validation stage.
        context: String,
        /// Human-readable failure details.
        details: String,
    },
    /// A caller-provided wire value was invalid.
    #[error("Invalid configuration for '{field}': {reason}")]
    InvalidConfiguration {
        /// Invalid field name.
        field: &'static str,
        /// Human-readable requirement.
        reason: String,
    },
}

impl Error {
    pub(crate) fn parse(context: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Parse {
            context: context.into(),
            details: details.into(),
        }
    }

    pub(crate) fn invalid_configuration(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidConfiguration {
            field,
            reason: reason.into(),
        }
    }
}

pub(super) fn header_validation_error(details: impl Into<String>) -> Error {
    Error::parse("Header validation", details)
}

pub(super) fn mismatched_version_error(actual: u32) -> Error {
    Error::Version {
        expected: IRSDK_VER as u32,
        found: actual,
    }
}

pub(super) fn variable_header_validation_error(details: impl Into<String>) -> Error {
    Error::parse("Variable header validation", details)
}
