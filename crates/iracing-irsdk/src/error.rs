//! Errors produced while decoding or validating the SDK wire contract.

use std::{any::type_name, mem::size_of};

use thiserror::Error;
use zerocopy::{
    TryFromBytes,
    error::{SizeError, TryReadError, ValidityError},
};

/// Result type for wire-contract operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error produced while decoding or validating SDK wire data.
#[derive(Debug, Error)]
pub enum Error {
    /// A byte buffer does not match a wire type's required size.
    #[error("Invalid wire size: expected {expected} bytes, received {actual}")]
    WireSize {
        /// Required wire representation size.
        expected: usize,
        /// Number of bytes supplied.
        actual: usize,
    },

    /// A byte buffer has the correct size but is not a valid value of the target type.
    #[error("Invalid wire value for {target}")]
    InvalidWireValue {
        /// Rust wire type that rejected the bytes.
        target: &'static str,
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

    /// An I/O error occurred while reading wire data.
    #[error("Wire I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub(crate) fn invalid_configuration(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidConfiguration {
            field,
            reason: reason.into(),
        }
    }
}

// -----------------------------------------------------------------------------
// Zerocopy conversions
// -----------------------------------------------------------------------------

/// Converts an incorrect buffer size into a wire-size error.
impl<T: Sized> From<SizeError<&[u8], T>> for Error {
    fn from(value: SizeError<&[u8], T>) -> Self {
        Self::WireSize {
            expected: size_of::<T>(),
            actual: value.into_src().len(),
        }
    }
}

/// Converts invalid byte representations into wire-value errors.
impl<T: TryFromBytes + Sized> From<ValidityError<&[u8], T>> for Error {
    fn from(_: ValidityError<&[u8], T>) -> Self {
        Self::InvalidWireValue {
            target: type_name::<T>(),
        }
    }
}

/// Converts fallible zerocopy reads into the corresponding domain error.
impl<T: TryFromBytes + Sized> From<TryReadError<&[u8], T>> for Error {
    fn from(value: TryReadError<&[u8], T>) -> Self {
        match value {
            TryReadError::Size(err) => err.into(),
            TryReadError::Validity(err) => err.into(),
            TryReadError::Alignment(never) => match never {},
        }
    }
}
