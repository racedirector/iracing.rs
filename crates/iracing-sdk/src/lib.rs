mod error;
#[cfg(feature = "codegen")]
mod codegen;
pub mod ibt;
pub mod schema;
pub mod types;
pub mod yaml_utils;

pub use error::*;
pub use ibt::IbtReader;
pub use schema::{SessionInfo, SessionInfoParser};
pub use types::*;

#[cfg(feature = "codegen")]
pub use codegen::session_root_schema;
#[cfg(all(feature = "codegen", feature = "schema-discovery"))]
pub use codegen::session_root_schema_with_discovery;

// Platform-specific modules
#[cfg(windows)]
pub mod windows;

// Windows memory exports
#[cfg(windows)]
pub use windows::{Connection as WindowsConnection, WaitResult};
