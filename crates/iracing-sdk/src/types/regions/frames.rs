use crate::{IRacingSDKError, Result};
use std::num::NonZeroUsize;

use super::{ByteRegion, FrameRegion};

/// Contiguous byte region containing zero or more fixed-size telemetry frames.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FramesRegion {
    region: ByteRegion,
    frame_size: NonZeroUsize,
    frame_count: usize,
}

impl FramesRegion {
    /// Creates a region containing fixed-size telemetry frames.
    ///
    /// # Errors
    ///
    /// Returns a parse error if `frame_size` is zero or if the region contains
    /// trailing bytes that do not form a complete frame.
    pub fn new(region: ByteRegion, frame_size: usize) -> Result<Self> {
        // Ensure the frame size is greater than 0
        let frame_size = NonZeroUsize::new(frame_size).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "FramesRegion::try_from",
                "Frame size must be greater than zero",
            )
        })?;

        ensure!(
            region.len() % frame_size == 0,
            IRacingSDKError::parse_error(
                "FramesRegion::new",
                format!(
                    "Frame region length {} is not divisible by frame size {}",
                    region.len(),
                    frame_size
                )
            )
        );

        let frame_count = region.len() / frame_size;

        Ok(Self {
            region,
            frame_size,
            frame_count,
        })
    }

    /// Returns the complete byte region containing the frames.
    pub fn as_region(self) -> ByteRegion {
        self.region
    }

    /// Returns the source-relative offset of the first frame byte.
    pub fn start(self) -> usize {
        self.region.offset()
    }

    /// Returns the exclusive source-relative end offset of the frame data.
    pub fn end(self) -> usize {
        self.region.end()
    }

    /// Returns the size of one frame in bytes.
    pub fn frame_size(&self) -> usize {
        self.frame_size.get()
    }

    /// Returns the number of complete frames in the region.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Returns the total length of the frame region in bytes.
    pub fn len(&self) -> usize {
        self.region.len()
    }

    /// Returns whether the region contains no frames.
    pub fn is_empty(&self) -> bool {
        self.frame_count == 0
    }

    /// Returns a `FrameRegion` derived from the supplied index.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the index is greater than or equal to the
    /// number of frames within the region, if the relative offset of the
    /// frame index overflows `usize`, or if adding the offset to the region
    /// offset overflows `usize`.
    pub fn frame(&self, index: usize) -> Result<FrameRegion> {
        ensure!(
            index < self.frame_count,
            IRacingSDKError::parse_error(
                "FramesRegion::frame",
                format!(
                    "Frame index {index} out of bounds for 0..{}",
                    self.frame_count
                ),
            )
        );

        let relative_offset = index.checked_mul(self.frame_size.get()).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "FramesRegion::frame",
                "Frame offset calculation overflowed",
            )
        })?;

        let offset = self
            .region
            .offset()
            .checked_add(relative_offset)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "FramesRegion::frame",
                    "Frame offset calculation overflowed",
                )
            })?;

        FrameRegion::new(offset, self.frame_size.get())
    }
}

impl TryFrom<(ByteRegion, usize)> for FramesRegion {
    type Error = IRacingSDKError;

    /// Creates a frame region from a byte region and frame size.
    ///
    /// # Errors
    ///
    /// Returns a parse error if `frame_size` is zero or if the region contains
    /// trailing bytes that do not form a complete frame.
    fn try_from((region, frame_size): (ByteRegion, usize)) -> Result<Self> {
        Self::new(region, frame_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_region_try_from_byte_region_and_size() {
        let valid_region = ByteRegion::try_from((0, 4)).unwrap();

        // Valid parsing
        assert!(FramesRegion::try_from((valid_region, 2)).is_ok());
        // Trailing bytes
        assert!(FramesRegion::try_from((valid_region, 3)).is_err());
        // Invalid frame size
        assert!(FramesRegion::try_from((valid_region, 0)).is_err());
    }

    #[test]
    fn frames_region_is_empty() {
        let frames_region = FramesRegion::new(ByteRegion::new(0, 0).unwrap(), 1).unwrap();

        assert!(frames_region.is_empty());
    }

    #[test]
    fn frames_region_length() {
        let frames_region = FramesRegion::new(ByteRegion::new(0, 4).unwrap(), 1).unwrap();

        assert_eq!(frames_region.len(), 4);
    }
}
