use thiserror::Error;

use crate::constants::IRSDK_VERSION;

/// Result type used by protocol parsing and conversion operations.
pub type Result<T, E = ProtocolError> = std::result::Result<T, E>;

/// Errors produced while decoding or validating iRacing protocol data.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// The iRacing SDK header version does not match the supported version.
    #[error("SDK version mismatch: expected {expected}, found {found}")]
    Version {
        /// The SDK version supported by this crate.
        expected: u32,
        /// The SDK version found in the protocol data.
        found: u32,
    },

    /// A memory access at the given offset was invalid or out of bounds.
    #[error("Memory access violation at offset {offset:#x}")]
    Memory {
        /// Byte offset at which the access violation occurred.
        offset: usize,
        /// Optional source error carrying additional context.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// A structured or textual protocol value could not be parsed.
    #[error("Parse error in {context}: {details}")]
    Parse {
        /// Human-readable description of the parsing stage that failed.
        context: String,
        /// Detailed description of the parse failure.
        details: String,
    },

    /// A protocol value could not be converted to the expected type.
    #[error("Type conversion error: {details}")]
    TypeConversion {
        /// Description of the failed conversion.
        details: String,
    },

    /// A caller-provided protocol configuration value was invalid.
    #[error("Invalid configuration for '{field}': {reason}")]
    InvalidConfiguration {
        /// Name of the invalid configuration field.
        field: &'static str,
        /// Human-readable explanation of the configuration requirement.
        reason: String,
    },

    /// A byte buffer does not match a wire type's required size.
    #[error("Invalid wire size: expected {expected} bytes, received {actual}")]
    WireSize {
        /// Required wire representation size.
        expected: usize,
        /// Number of bytes supplied.
        actual: usize,
    },
}

impl ProtocolError {
    /// Creates a protocol parsing error.
    pub fn parse_error(context: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Parse {
            context: context.into(),
            details: details.into(),
        }
    }

    /// Creates an error for an unsupported SDK protocol version.
    pub fn mismatched_version_error(actual: u32) -> Self {
        ProtocolError::Version {
            expected: IRSDK_VERSION as u32,
            found: actual,
        }
    }

    /// Creates an out-of-bounds memory access error.
    pub fn memory_access_error(offset: usize) -> Self {
        Self::Memory {
            offset,
            source: None,
        }
    }

    /// Creates a protocol type conversion error.
    pub fn type_conversion(
        expected: impl std::fmt::Display,
        actual: impl std::fmt::Display,
    ) -> Self {
        Self::TypeConversion {
            details: format!("expected {expected}, found {actual}"),
        }
    }

    /// Creates an invalid protocol configuration error.
    pub fn invalid_configuration(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidConfiguration {
            field,
            reason: reason.into(),
        }
    }
}
