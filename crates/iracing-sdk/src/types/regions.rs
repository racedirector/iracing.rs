use crate::{
    IRacingSDKError, Result, SessionInfoBuffer, VariableHeadersBuffer,
    irsdk::{Header, VariableHeader},
};
use std::ops::Range;

/// Offset and length for a byte span within an SDK data source.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ByteRegion {
    /// Start offset of the region, measured in bytes from the source origin.
    pub offset: usize,
    /// Length of the region in bytes.
    pub length: usize,
}

impl ByteRegion {
    /// Returns whether the header advertised a nonzero session-information region.
    pub fn is_valid(&self) -> bool {
        self.offset > 0 && self.length > 0
    }

    /// Returns the region as a half-open byte range.
    ///
    /// This performs unchecked addition for the range end. Use
    /// [`Self::as_checked_range`] when handling untrusted offsets or lengths.
    pub fn as_range(&self) -> Range<usize> {
        self.offset..self.offset + self.length
    }

    /// Returns the region as a half-open byte range with overflow checking.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the end offset overflows `usize`.
    pub fn as_checked_range(&self) -> Result<Range<usize>> {
        let offset = self.offset;
        let length = self.length;

        let end = offset.checked_add(length).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "ByteRegion",
                format!("Region offset {offset} + length {length} overflows usize"),
            )
        })?;

        Ok(offset..end)
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait ByteParser {
    fn bytes_at_region(&self, region: ByteRegion) -> &[u8];
}

fn checked_range(
    region: ByteRegion,
    data_len: usize,
    context: &'static str,
) -> Result<Range<usize>> {
    let range = region.as_checked_range()?;

    if range.end > data_len {
        return Err(IRacingSDKError::parse_error(
            context,
            format!(
                "Region {}..{} exceeds data length {data_len}",
                range.start, range.end
            ),
        ));
    }

    Ok(range)
}

/// Location and size of the session-information region advertised by a [`Header`].  
///  
/// Construction validates the offset and length conversions, but does not check  
/// that the region fits in a source. Use [`Self::checked_range`], [`Self::bytes`],  
/// or [`Self::buffer`] to validate source bounds before accessing the region.  
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionInfoRegion(ByteRegion);

impl SessionInfoRegion {
    /// Returns the associated byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.0
    }

    /// Returns the region's half-open byte range within a source of `data_len` bytes.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the end offset overflows `usize` or exceeds
    /// `data_len`.
    pub fn checked_range(&self, data_len: usize) -> Result<Range<usize>> {
        checked_range(
            self.as_region(),
            data_len,
            "SessionInfoRegion::checked_range",
        )
    }

    /// Borrows the session-information bytes from the complete source without copying.
    ///
    /// `source` must begin at the origin used by the underlying [`ByteRegion`].
    ///
    /// # Errors
    ///
    /// Returns a parse error if the region's end offset overflows `usize` or
    /// the region extends beyond `source`.
    pub fn bytes<'a>(&self, source: &'a [u8]) -> Result<&'a [u8]> {
        Ok(&source[self.checked_range(source.len())?])
    }

    /// Copies the session-information bytes into an owned buffer.
    ///
    /// `source` must begin at the origin used by the underlying [`ByteRegion`].
    /// The returned buffer is independent of `source`.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the region's end offset overflows `usize` or
    /// the region extends beyond `source`.
    pub fn buffer(&self, source: &[u8]) -> Result<SessionInfoBuffer> {
        Ok(SessionInfoBuffer::from_checked_region(self.bytes(source)?))
    }

    /// Returns whether the header advertised a nonzero variable-header region.
    pub fn is_valid(&self) -> bool {
        self.as_region().is_valid()
    }
}

impl TryFrom<&Header> for SessionInfoRegion {
    type Error = IRacingSDKError;

    fn try_from(value: &Header) -> Result<Self> {
        Ok(SessionInfoRegion(ByteRegion {
            offset: usize::try_from(value.session_info_offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "SessionInfoRegion::try_from",
                    format!("Could not convert {} to usize", value.session_info_offset),
                )
            })?,
            length: usize::try_from(value.session_info_length).map_err(|_| {
                IRacingSDKError::parse_error(
                    "SessionInfoRegion::try_from",
                    format!("Could not convert {} to usize", value.session_info_length),
                )
            })?,
        }))
    }
}

/// Location and size of the variable-header region advertised by a [`Header`].
///
/// Construct with [`TryFrom<&Header>`](TryFrom::try_from). Construction validates
/// the offset, count, and byte-length calculation, but does not check that the
/// region fits in a source. Use [`Self::checked_range`], [`Self::bytes`], or
/// [`Self::buffer`] to validate source bounds before accessing the region.
/// Individual variable headers are not semantically validated by this type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VariableHeaderRegion {
    /// Byte span containing the contiguous variable-header records.
    pub region: ByteRegion,
    /// Number of variable-header records advertised by the source header.
    pub count: usize,
}

impl VariableHeaderRegion {
    /// Returns the associated byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the number of variable headers advertised by the source header.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns the region's half-open byte range within a source of `data_len` bytes.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the end offset overflows `usize` or exceeds
    /// `data_len`.
    pub fn checked_range(&self, data_len: usize) -> Result<Range<usize>> {
        checked_range(self.region, data_len, "VariableHeaderRegion::checked_range")
    }

    /// Borrows the region's bytes from the complete source without copying.
    ///
    /// `source` must begin at the origin used by [`Self::region`].
    ///
    /// # Errors
    ///
    /// Returns a parse error if the region's end offset overflows `usize` or
    /// the region extends beyond `source`.
    pub fn bytes<'a>(&self, source: &'a [u8]) -> Result<&'a [u8]> {
        Ok(&source[self.checked_range(source.len())?])
    }

    /// Copies the region's bytes into an owned variable-header snapshot.
    ///
    /// `source` must begin at the origin used by [`Self::region`]. The returned
    /// buffer is independent of `source`; individual headers are not
    /// semantically validated.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the region's end offset overflows `usize`, the
    /// region extends beyond `source`, or the bytes do not decode to exactly
    /// the advertised number of complete variable headers.
    pub fn buffer(&self, source: &[u8]) -> Result<VariableHeadersBuffer> {
        VariableHeadersBuffer::try_from_region_bytes(self.bytes(source)?, self.count)
    }

    /// Returns whether the header advertised a nonzero variable-header region.
    pub fn is_valid(&self) -> bool {
        self.region.is_valid()
    }
}

impl TryFrom<&Header> for VariableHeaderRegion {
    type Error = IRacingSDKError;

    /// Derives the region from the header's variable offset and count.
    ///
    /// Source bounds are checked separately by [`Self::checked_range`].
    ///
    /// # Errors
    ///
    /// Returns a parse error if the offset or count cannot be represented as
    /// `usize`, or if the count times the variable-header wire size overflows.
    fn try_from(value: &Header) -> Result<Self> {
        let offset = usize::try_from(value.variable_header_offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableHeaderRegion::try_from",
                format!(
                    "Could not convert {} to usize",
                    value.variable_header_offset
                ),
            )
        })?;

        let count = usize::try_from(value.variable_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "VariableHeaderRegion::try_from",
                format!("Could not convert {} to usize", value.variable_count),
            )
        })?;

        let length = count
            .checked_mul(size_of::<VariableHeader>())
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableHeaderRegion::try_from",
                    "Variable headers size calculation overflowed".to_string(),
                )
            })?;

        Ok(VariableHeaderRegion {
            region: ByteRegion { offset, length },
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionInfoRegion, VariableHeaderRegion, checked_range};
    use crate::{
        irsdk::{Header, VariableHeader},
        types::regions::ByteRegion,
    };
    use zerocopy::FromZeros;

    #[test]
    fn session_region_rejects_negative_header_fields() {
        for (offset, length) in [(-1, 1), (1, -1)] {
            let mut header = Header::new_zeroed();
            header.session_info_offset = offset;
            header.session_info_length = length;
            assert!(SessionInfoRegion::try_from(&header).is_err());
        }
    }

    #[test]
    fn session_region_copies_only_advertised_bytes() {
        let mut header = Header::new_zeroed();
        header.session_info_offset = 4;
        header.session_info_length = 7;
        let region = SessionInfoRegion::try_from(&header).unwrap();
        let mut source = b"skipSessionpadding".to_vec();
        assert_eq!(region.bytes(&source).unwrap(), b"Session");
        let buffer = region.buffer(&source).unwrap();
        source.fill(0);
        assert_eq!(String::from(buffer), "Session");
    }

    #[test]
    fn session_region_requires_the_full_region_even_with_early_nul() {
        let region = SessionInfoRegion(ByteRegion {
            offset: 1,
            length: 4,
        });

        assert!(region.buffer(b"x\0").is_err());
        assert!(region.buffer(b"x\0pad").is_ok());
    }

    #[test]
    fn variable_header_region_rejects_negative_count() {
        let mut header = Header::new_zeroed();
        header.variable_count = -1;

        assert!(VariableHeaderRegion::try_from(&header).is_err());
    }

    #[test]
    fn variable_header_region_rejects_negative_offset() {
        let mut header = Header::new_zeroed();
        header.variable_header_offset = -1;

        assert!(VariableHeaderRegion::try_from(&header).is_err());
    }

    #[test]
    fn checked_range_accepts_region_within_data() {
        let region = ByteRegion {
            offset: 4,
            length: 6,
        };
        assert_eq!(checked_range(region, 10, "test").unwrap(), 4..10);
    }

    #[test]
    fn checked_range_rejects_region_beyond_data() {
        let region = ByteRegion {
            offset: 4,
            length: 7,
        };

        assert!(checked_range(region, 10, "test").is_err());
    }

    #[test]
    fn checked_range_rejects_endpoint_overflow() {
        let region = ByteRegion {
            offset: usize::MAX,
            length: 1,
        };

        assert!(checked_range(region, usize::MAX, "test").is_err());
    }

    #[test]
    fn variable_header_region_extracts_bytes_and_owned_buffer() {
        let region = VariableHeaderRegion {
            region: ByteRegion {
                offset: 4,
                length: size_of::<VariableHeader>(),
            },
            count: 1,
        };
        let source = vec![0; 4 + size_of::<VariableHeader>()];

        assert_eq!(
            region.bytes(&source).unwrap().len(),
            size_of::<VariableHeader>()
        );
        assert_eq!(region.buffer(&source).unwrap().iter().len(), 1);
    }

    #[test]
    fn variable_header_region_rejects_short_source() {
        let region = VariableHeaderRegion {
            region: ByteRegion {
                offset: 4,
                length: size_of::<VariableHeader>(),
            },
            count: 1,
        };

        assert!(region.bytes(&[0; 4]).is_err());
        assert!(region.buffer(&[0; 4]).is_err());
    }

    #[test]
    fn variable_header_region_checks_advertised_count_during_buffer_construction() {
        let region = VariableHeaderRegion {
            region: ByteRegion {
                offset: 0,
                length: size_of::<VariableHeader>(),
            },
            count: 2,
        };
        let source = vec![0; size_of::<VariableHeader>()];

        assert!(region.buffer(&source).is_err());
    }
}
