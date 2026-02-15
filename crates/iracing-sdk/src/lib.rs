mod error;
pub mod ibt;
pub mod schema;
pub mod types;

pub use error::*;
pub use ibt::IbtReader;
pub use types::*;

// Platform-specific modules
#[cfg(windows)]
pub mod windows;

// Windows memory exports
#[cfg(windows)]
pub use windows::{Connection as WindowsConnection, WaitResult};
