//! IBT file reading and parsing support (cross-platform)
//!
//! This module provides support for reading iRacing's IBT (telemetry) files
//! and implementing the FrameProvider interface for unified telemetry streaming.

pub mod format;
pub mod reader;
pub mod writer;

pub use reader::IbtReader;
pub use writer::{FrameProjection, IbtWriteOptions, IbtWriter};
