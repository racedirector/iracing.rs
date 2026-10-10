//! IBT file reading and parsing support (cross-platform)
//!
//! This module provides support for reading iRacing's IBT (telemetry) files
//! through indexed frame reads and owned metadata snapshots.

mod file;
pub mod reader;
pub mod replay;

pub use replay::IbtReplay;

pub use file::IbtFile;

pub use reader::{IbtFrames, IbtReader, RecordedFrame};

#[cfg(test)]
mod tests;
