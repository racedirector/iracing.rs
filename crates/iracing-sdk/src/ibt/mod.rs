//! IBT file reading and parsing support (cross-platform)
//!
//! This module provides support for reading iRacing's IBT (telemetry) files
//! and implementing the FrameProvider interface for unified telemetry streaming.

pub mod format;
/// Lap-oriented indexing helpers built on top of [`IbtReader`].
pub mod lap_index;
pub mod reader;

pub use lap_index::{IndexedIbt, LapFlags, LapFrameIter, LapIndex, LapIndexVars, LapRef};
pub use reader::IbtReader;
