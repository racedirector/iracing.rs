use thiserror::Error;
use windows::core::Error as WindowsError;

#[derive(Error, Debug)]
pub enum BroadcastError {
    #[error("Failed to connect to iRacing: {reason}")]
    Connection { reason: String },

    #[error("Windows API error")]
    Windows(#[from] WindowsError),

    #[error("Command validation error: {reason}")]
    Validation { reason: String },

    #[error("Wide-string conversaion error")]
    Conversion(#[from] widestring::error::ContainsNul<u16>),
}

pub type Result<T, E = BroadcastError> = std::result::Result<T, E>;
