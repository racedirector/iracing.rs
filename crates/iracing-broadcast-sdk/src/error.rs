#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to connect to iRacing: {reason}")]
    Connection { reason: String },

    #[cfg(windows)]
    #[error("Windows API error")]
    Windows(#[from] windows::core::Error),

    #[error("Command validation error: {reason}")]
    Validation { reason: String },

    #[error("Wide-string conversion error")]
    Conversion(#[from] widestring::error::ContainsNul<u16>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
