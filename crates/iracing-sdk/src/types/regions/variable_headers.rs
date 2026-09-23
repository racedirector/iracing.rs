use iracing_irsdk::{Header, VariableHeader};

use super::ByteRegion;
use crate::{IRacingSDKError, Result};

/// Location and size of the variable-header region advertised by a [`Header`].
///
/// Construct with [`TryFrom<&Header>`](TryFrom::try_from). Construction validates
/// the offset, count, and byte-length calculation, but does not check that the
/// region fits in a source. Compare [`Self::end`] with the source length before
/// accessing the region.
/// Individual variable headers are not semantically validated by this type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VariableHeadersRegion {
    /// Byte span containing the contiguous variable-header records.
    region: ByteRegion,
    /// Number of variable-header records advertised by the source header.
    count: usize,
}

impl VariableHeadersRegion {
    /// Derives the variable-header region advertised by an SDK header.
    ///
    /// Returns `Ok(None)` when the header advertises zero variable headers.
    /// Source bounds are not checked; callers that have a complete source
    /// should compare [`Self::end`] with its length before slicing.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the advertised offset or count cannot be
    /// represented as `usize`, or if the region-size calculation overflows.
    pub fn try_from_header(header: &Header) -> Result<Option<Self>> {
        let offset = usize::try_from(header.variable_header_offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableHeadersRegion::try_from",
                format!(
                    "Could not convert {} to usize",
                    header.variable_header_offset
                ),
            )
        })?;

        let count = usize::try_from(header.variable_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableHeadersRegion::try_from",
                format!("Could not convert {} to usize", header.variable_count),
            )
        })?;

        let length = count
            .checked_mul(size_of::<VariableHeader>())
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableHeadersRegion::try_from",
                    "Variable headers size calculation overflowed".to_string(),
                )
            })?;

        if length == 0 {
            return Ok(None);
        }

        Ok(Some(Self {
            region: ByteRegion::new(offset, length)?,
            count,
        }))
    }

    /// Returns the byte region containing the variable-header records.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the number of variable-header records in the region.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns the source-relative starting byte offset.
    pub fn offset(self) -> usize {
        self.region.offset()
    }

    /// Returns the length of the variable-header region in bytes.
    pub fn len(self) -> usize {
        self.region.len()
    }

    /// Returns whether the variable-header region contains no bytes.
    pub fn is_empty(self) -> bool {
        self.region.is_empty()
    }

    /// Returns the exclusive source-relative end offset.
    pub fn end(&self) -> usize {
        self.region.end()
    }

    /// Returns whether each region begins before the other region ends.
    pub fn overlaps(&self, other: ByteRegion) -> bool {
        self.region.overlaps(other)
    }
}

impl TryFrom<&Header> for VariableHeadersRegion {
    type Error = IRacingSDKError;

    /// Derives the region from the header's variable offset and count.
    ///
    /// Source bounds must be checked separately using [`Self::end`].
    ///
    /// # Errors
    ///
    /// Returns a parse error if the offset or count cannot be represented as
    /// `usize`, or if the count times the variable-header wire size overflows.
    fn try_from(value: &Header) -> Result<Self> {
        match Self::try_from_header(value)? {
            Some(region) => Ok(region),
            None => Err(IRacingSDKError::parse_error(
                "VariableHeadersRegion::try_from",
                "Header advertised empty variable headers region",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::FromZeros;

    #[test]
    fn variable_header_region_rejects_negative_count() {
        let mut header = Header::new_zeroed();
        header.variable_count = -1;

        assert!(VariableHeadersRegion::try_from(&header).is_err());
    }

    #[test]
    fn variable_header_region_rejects_negative_offset() {
        let mut header = Header::new_zeroed();
        header.variable_header_offset = -1;

        assert!(VariableHeadersRegion::try_from(&header).is_err());
    }
}
