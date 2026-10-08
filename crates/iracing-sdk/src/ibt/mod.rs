//! IBT file reading and parsing support (cross-platform)
//!
//! This module provides support for reading iRacing's IBT (telemetry) files
//! through indexed frame reads and owned metadata snapshots.

pub mod reader;

pub use reader::{IbtFrames, IbtReader, RecordedFrame};

#[cfg(test)]
mod tests;
