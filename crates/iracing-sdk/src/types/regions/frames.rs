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
                "FramesRegion::new",
                "Frame size must be greater than zero",
            )
        })?;

        if region.len() % frame_size != 0 {
            return Err(IRacingSDKError::parse_error(
                "FramesRegion::new",
                format!(
                    "Frame region length {} is not divisible by frame size {}",
                    region.len(),
                    frame_size
                ),
            ));
        }

        let frame_count = region.len() / frame_size;

        Ok(Self {
            region,
            frame_size,
            frame_count,
        })
    }

    /// Returns the complete byte region containing the frames.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the source-relative offset of the first frame byte.
    pub fn start(&self) -> usize {
        self.region.offset()
    }

    /// Returns the exclusive source-relative end offset of the frame data.
    pub fn end(&self) -> usize {
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
        if index >= self.frame_count {
            return Err(IRacingSDKError::parse_error(
                "FramesRegion::frame",
                format!(
                    "Frame index {index} out of bounds for 0..{}",
                    self.frame_count
                ),
            ));
        }

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
    fn indexed_frames_stay_within_region_near_usize_limit() {
        let frames = FramesRegion::new(ByteRegion::new(usize::MAX - 8, 8).unwrap(), 4).unwrap();
        assert_eq!(frames.frame(0).unwrap().offset(), usize::MAX - 8);
        assert_eq!(frames.frame(1).unwrap().end(), usize::MAX);
        assert!(frames.frame(2).is_err());
        assert!(frames.frame(usize::MAX).is_err());
        assert!(ByteRegion::new(usize::MAX - 7, 8).is_err());
    }

    #[test]
    fn overflowing_index_is_rejected_before_multiplication() {
        let frames = FramesRegion::new(ByteRegion::new(0, usize::MAX - 1).unwrap(), 2).unwrap();
        let index = usize::MAX / 2 + 1;
        assert!(index.checked_mul(2).is_none());
        assert!(frames.frame(index).is_err());
        assert!(frames.frame(frames.frame_count()).is_err());
    }

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
    fn rejects_zero_frame_size() {
        assert!(FramesRegion::new(ByteRegion::new(0, 0).unwrap(), 0).is_err());
    }

    #[test]
    fn rejects_region_not_divisible_by_frame_size() {
        assert!(FramesRegion::new(ByteRegion::new(0, 2).unwrap(), 3).is_err());
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

    #[test]
    fn frame_rejects_index_greater_than_frame_count() {
        let frames_region = FramesRegion::new(ByteRegion::new(0, 5).unwrap(), 1).unwrap();
        assert!(frames_region.frame(6).is_err());
    }
}
