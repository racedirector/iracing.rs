use iracing_irsdk::VariableHeader;
use std::ops::Range;

use crate::{IRacingSDKError, Result, VariableInfo};

use super::ByteRegion;

/// Frame-relative byte region occupied by one telemetry variable.
pub struct VariableRegion {
    region: ByteRegion,
}

impl VariableRegion {
    /// Returns the underlying frame-relative byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the region as a half-open byte range.
    pub fn as_range(&self) -> Range<usize> {
        self.as_region().as_range()
    }

    /// Returns whether the variable occupies zero bytes.
    pub fn is_empty(&self) -> bool {
        self.as_region().is_empty()
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

        let length = value
            .variable_type
            .byte_size()
            .checked_mul(count)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableRegion::try_from",
                    "Variable region size calculation overflowed",
                )
            })?;

        Ok(Self {
            region: ByteRegion::new(offset, length)?,
        })
    }
}

impl TryFrom<&VariableInfo> for VariableRegion {
    type Error = IRacingSDKError;

    /// Derives the frame-relative region described by parsed variable metadata.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the variable type has no storage width or if a
    /// size calculation overflows `usize`.
    fn try_from(value: &VariableInfo) -> Result<Self> {
        let length = value
            .data_type
            .byte_size()
            .checked_mul(value.count)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableRegion::try_from",
                    "Variable region size calculation overflowed",
                )
            })?;

        Ok(Self {
            region: ByteRegion::new(value.offset, length)?,
        })
    }
}
