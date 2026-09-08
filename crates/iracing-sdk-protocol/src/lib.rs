#![warn(missing_docs)]
//! Types and parsers for the iRacing SDK wire protocol.

#[cfg(feature = "codegen")]
mod codegen;
mod error;
pub mod irsdk;
mod parse_utils;
pub mod session;

pub use error::{ProtocolError, Result};
pub use irsdk::*;
