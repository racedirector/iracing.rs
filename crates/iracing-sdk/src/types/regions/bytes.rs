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
        // Ensure self is not empty...
        !self.is_empty()
            // Other is not empty...
            && !other.is_empty()
            // Overlap
            && self.offset < other.end()
            && other.offset < self.end()
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
    fn empty_range_succeeds() {
        assert!(ByteRegion::new(0, 0).is_ok());
    }

    #[test]
    fn end_calculation_succeeds() {
        let region = ByteRegion::new(0, 2).unwrap();

        assert_eq!(region.end(), 2);
    }

    #[test]
    fn usize_overflow_rejected() {
        assert!(ByteRegion::new(usize::MAX, 1).is_err());
    }

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

    #[allow(clippy::reversed_empty_ranges)]
    #[test]
    fn try_from_range_rejects_end_preceding_start() {
        assert!(ByteRegion::try_from(12..11).is_err());
    }

    #[test]
    fn try_from_tuple() {
        // `usize` overflow
        assert!(ByteRegion::try_from((usize::MAX, 1)).is_err());
        assert!(ByteRegion::try_from((0, 100)).is_ok());
    }

    #[test]
    fn overlap_half_open() {
        // 0..4
        let region = ByteRegion::new(0, 4).unwrap();
        // 2..5
        let overlap = ByteRegion::new(2, 3).unwrap();

        assert!(region.overlaps(overlap));

        // 4..5
        let adjacent = ByteRegion::new(4, 1).unwrap();
        assert!(!region.overlaps(adjacent));
    }

    #[test]
    fn empty_region_never_overlaps_even_inside_another_region() {
        let occupied = ByteRegion::new(2, 5).unwrap();
        for offset in [2, 4, 7] {
            let empty = ByteRegion::new(offset, 0).unwrap();
            assert!(!empty.overlaps(occupied));
            assert!(!occupied.overlaps(empty));
        }
    }

    #[test]
    fn valid_regions_can_end_at_usize_max() {
        for region in [
            ByteRegion::new(usize::MAX, 0).unwrap(),
            ByteRegion::new(usize::MAX - 1, 1).unwrap(),
        ] {
            assert_eq!(region.end(), usize::MAX);
            assert_eq!(region.as_range().end, usize::MAX);
        }
        assert!(ByteRegion::new(usize::MAX, 1).is_err());
    }
}
