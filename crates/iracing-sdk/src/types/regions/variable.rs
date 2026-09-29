use iracing_irsdk::VariableHeader;
use std::{num::NonZeroUsize, ops::Range};

use crate::{IRacingSDKError, Result, VariableInfo};

use super::ByteRegion;

/// Frame-relative byte region occupied by one telemetry variable.
#[derive(Debug, Copy, Clone)]
pub struct VariableRegion {
    region: ByteRegion,
    count: NonZeroUsize,
}

impl VariableRegion {
    /// Creates a variable region contained within a frame.
    ///
    /// # Errors
    ///
    /// Returns a parse error if either dimension is zero, a size calculation
    /// overflows, or the region extends past `frame_size`.
    pub fn try_new(
        offset: usize,
        element_size: usize,
        count: usize,
        frame_size: usize,
    ) -> Result<Self> {
        let count = NonZeroUsize::new(count).ok_or_else(|| {
            IRacingSDKError::parse_error("VariableRegion", "Variable count must be positive")
        })?;
        let element_size = NonZeroUsize::new(element_size).ok_or_else(|| {
            IRacingSDKError::parse_error("VariableRegion", "Element size must be positive")
        })?;

        let length = element_size.get().checked_mul(count.get()).ok_or_else(|| {
            IRacingSDKError::parse_error("VariableRegion", "Variable region length overflows usize")
        })?;

        let region = ByteRegion::new(offset, length)?;
        if region.end() > frame_size {
            return Err(IRacingSDKError::parse_error(
                "VariableRegion",
                format!(
                    "Variable region ends at {}, past frame size {frame_size}",
                    region.end()
                ),
            ));
        }

        Ok(Self { region, count })
    }

    /// Returns the underlying frame-relative byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the region as a half-open byte range.
    pub fn as_range(&self) -> Range<usize> {
        self.region.as_range()
    }

    /// Returns whether the variable occupies zero bytes.
    pub fn is_empty(&self) -> bool {
        self.region.is_empty()
    }

    /// Returns the frame-relative starting byte offset.
    pub fn offset(&self) -> usize {
        self.region.offset()
    }

    /// Returns the region length in bytes.
    pub fn len(&self) -> usize {
        self.region.len()
    }

    /// Returns the number of elements in the region.
    pub fn count(&self) -> usize {
        self.count.get()
    }
}

impl TryFrom<&VariableHeader> for VariableRegion {
    type Error = IRacingSDKError;

    /// Derives the frame-relative region described by a wire variable header.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the offset, element count, or variable type is
    /// invalid, if the type has no storage width, or if a size calculation
    /// overflows `usize`.
    fn try_from(value: &VariableHeader) -> Result<Self> {
        let offset = usize::try_from(value.offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableRegion::try_from",
                format!("Could not convert {} to usize", value.offset),
            )
        })?;

        let count = usize::try_from(value.count).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableRegion::try_from",
                format!("Could not convert {} to usize", value.count,),
            )
        })?;
        let count = NonZeroUsize::new(count).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "VariableRegion::try_from",
                "Variable count must be positive",
            )
        })?;

        let length = value
            .variable_type
            .byte_size()
            .checked_mul(count.get())
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableRegion::try_from",
                    "Variable region size calculation overflowed",
                )
            })?;

        let region = ByteRegion::new(offset, length)?;
        Ok(Self { region, count })
    }
}

impl TryFrom<&VariableInfo> for VariableRegion {
    type Error = IRacingSDKError;

    /// Returns the geometry already validated during metadata construction.
    fn try_from(value: &VariableInfo) -> Result<Self> {
        Ok(value.region())
    }
}

#[cfg(test)]
mod tests {
    use super::VariableRegion;

    #[test]
    fn scalar_region_at_frame_start() {
        let region = VariableRegion::try_new(0, 4, 1, 4).unwrap();

        assert_eq!(region.offset(), 0);
        assert_eq!(region.len(), 4);
        assert_eq!(region.count(), 1);
    }

    #[test]
    fn scalar_region_at_nonzero_offset() {
        let region = VariableRegion::try_new(4, 2, 1, 8).unwrap();

        assert_eq!(region.as_range(), 4..6);
        assert_eq!(region.count(), 1);
    }

    #[test]
    fn array_region_has_complete_extent() {
        let region = VariableRegion::try_new(8, 4, 3, 20).unwrap();

        assert_eq!(region.as_region().as_range(), 8..20);
        assert_eq!(region.count(), 3);
    }

    #[test]
    fn zero_geometry_is_rejected() {
        assert!(VariableRegion::try_new(0, 4, 0, 4).is_err());
        assert!(VariableRegion::try_new(0, 0, 1, 4).is_err());
    }

    #[test]
    fn checked_arithmetic_is_required() {
        assert!(VariableRegion::try_new(0, usize::MAX, 2, usize::MAX).is_err());
        assert!(VariableRegion::try_new(usize::MAX - 1, 4, 1, usize::MAX).is_err());
    }

    #[test]
    fn exact_frame_end_is_valid_but_past_end_is_not() {
        assert!(VariableRegion::try_new(8, 4, 3, 20).is_ok());
        assert!(VariableRegion::try_new(9, 4, 3, 20).is_err());
    }
}
