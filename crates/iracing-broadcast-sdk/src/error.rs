//! Failures at command encoding and native broadcast initialization/dispatch.

/// A broadcast encoding or transport failure, without simulator acknowledgment.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Native message registration failed; this does not report simulator status.
    #[error("Failed to connect to iRacing: {reason}")]
    Connection { reason: String },

    #[cfg(windows)]
    /// A Win32 dispatch operation failed.
    #[error("Windows API error")]
    Windows(#[from] windows::core::Error),

    /// Typed command arguments cannot be encoded under this SDK's checks.
    #[error("Command validation error: {reason}")]
    Validation { reason: String },

    /// A message registration string contains an interior NUL.
    #[error("Wide-string conversion error")]
    Conversion(#[from] widestring::error::ContainsNul<u16>),
}

/// Result of broadcast encoding or transport operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;
