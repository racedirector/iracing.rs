use crate::{IRacingSDKError, Result};
use std::ops::Range;

/// Offset and length for a byte span within an SDK data source.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ByteRegion {
    /// Start offset of the region, measured in bytes from the source origin.
    offset: usize,
    /// Length of the region in bytes.
    length: usize,
}

impl ByteRegion {
    /// Creates a new byte region.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Parse`] if `offset + length` overflows
    /// `usize`.
    pub fn new(offset: usize, length: usize) -> Result<Self> {
        offset.checked_add(length).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "ByteRegion",
                format!("Region offset {offset} + length {length} overflows usize"),
            )
        })?;

        Ok(Self { offset, length })
    }

    /// Returns the source-relative starting byte offset.
    pub fn offset(self) -> usize {
        self.offset
    }

    /// Returns the length of the region in bytes.
    pub fn len(self) -> usize {
        self.length
    }

    /// Returns whether the region contains no bytes.
    pub fn is_empty(self) -> bool {
        self.length == 0
    }

    /// Returns the exclusive end offset of the region.
    pub fn end(&self) -> usize {
        self.offset + self.length
    }

    /// Returns the region as a half-open byte range.
    ///
    /// Construction guarantees that calculating the range end cannot overflow.
    pub fn as_range(&self) -> Range<usize> {
        self.offset..self.end()
    }

    /// Returns whether each region begins before the other region ends.
    pub fn overlaps(&self, other: Self) -> bool {
        self.offset < other.end() && other.offset < self.end()
    }
}

impl TryFrom<(usize, usize)> for ByteRegion {
    type Error = IRacingSDKError;

    /// Creates a region from an `(offset, length)` pair.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Parse`] if `offset + length` overflows
    /// `usize`.
    fn try_from((offset, length): (usize, usize)) -> Result<Self> {
        Self::new(offset, length)
    }
}

impl TryFrom<Range<usize>> for ByteRegion {
    type Error = IRacingSDKError;

    /// Creates a region from a half-open byte range.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Parse`] if the range ends before it starts.
    fn try_from(range: Range<usize>) -> Result<Self> {
        let length = range.end.checked_sub(range.start).ok_or_else(|| {
            IRacingSDKError::parse_error("ByteRegion::try_from", "Range end precedes range start")
        })?;

        Self::new(range.start, length)
    }
}

impl From<ByteRegion> for Range<usize> {
    fn from(value: ByteRegion) -> Self {
        value.as_range()
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

    use crate::ByteRegion;

    #[test]
    fn try_from_range() {
        assert!(
            ByteRegion::try_from(std::ops::Range {
                start: 200,
                end: 100,
            })
            .is_err()
        );

        let valid_region_result = ByteRegion::try_from(1..100);
        assert!(valid_region_result.is_ok());
        let valid_region = valid_region_result.unwrap();
        assert_eq!(valid_region.offset, 1);
        assert_eq!(valid_region.length, 99);
    }

    #[test]
    fn try_from_tuple() {
        // `usize` overflow
        assert!(ByteRegion::try_from((usize::MAX, 1)).is_err());
        assert!(ByteRegion::try_from((0, 100)).is_ok());
    }
}
