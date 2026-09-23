use iracing_irsdk::{Header, VariableBuffer};

use crate::{IRacingSDKError, Result};

use super::ByteRegion;

/// Location of exactly one telemetry frame within an SDK data source.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FrameRegion(ByteRegion);

impl FrameRegion {
    /// Wraps a byte region that has already been established as one complete frame.
    pub(crate) fn new(offset: usize, frame_size: usize) -> Result<Self> {
        Ok(Self(ByteRegion::new(offset, frame_size)?))
    }

    /// Returns the underlying byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.0
    }

    /// Returns the source-relative starting byte offset of the frame.
    pub fn offset(self) -> usize {
        self.0.offset()
    }

    /// Returns the frame size in bytes.
    pub fn len(self) -> usize {
        self.0.len()
    }

    /// Returns whether the frame contains no bytes.
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// Returns the exclusive source-relative end offset of the frame.
    pub fn end(self) -> usize {
        self.0.end()
    }
}

impl TryFrom<(&VariableBuffer, &Header)> for FrameRegion {
    type Error = IRacingSDKError;

    /// Derives a frame region from a variable buffer and its SDK header.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the advertised buffer offset or frame length
    /// cannot be represented as `usize`, or if their sum overflows `usize`.
    fn try_from((buffer, header): (&VariableBuffer, &Header)) -> Result<Self> {
        let offset = usize::try_from(buffer.buffer_offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "FrameRegion::try_from",
                format!("Could not convert {} to usize", buffer.buffer_offset),
            )
        })?;

        let length = usize::try_from(header.buffer_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "FrameRegion::try_from",
                format!("Could not convert {} to usize", buffer.buffer_offset),
            )
        })?;

        Self::new(offset, length)
    }
}
