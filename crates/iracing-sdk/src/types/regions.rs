use crate::{
    IRacingSDKError, Result, SessionInfoBuffer, VariableHeadersBuffer,
    irsdk::{Header, VariableHeader, WireType},
};
use std::ops::Range;

fn checked_range(
    offset: usize,
    length: usize,
    data_len: usize,
    context: &'static str,
) -> Result<Range<usize>> {
    let end = offset.checked_add(length).ok_or_else(|| {
        IRacingSDKError::parse_error(
            context,
            format!("Region offset {offset} + length {length} overflows usize"),
        )
    })?;

    if end > data_len {
        return Err(IRacingSDKError::parse_error(
            context,
            format!("Region {offset}..{end} exceeds data length {data_len}"),
        ));
    }

    Ok(offset..end)
}

pub struct SessionInfoRegion {
    offset: usize,
    length: usize,
}

impl SessionInfoRegion {
    /// Returns the region's byte offset from the beginning of the source.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the region's byte length: the header count times the wire size
    /// of a [`VariableHeader`].
    pub fn length(&self) -> usize {
        self.length
    }

    pub fn checked_range(&self, data_len: usize) -> Result<Range<usize>> {
        checked_range(
            self.offset,
            self.length,
            data_len,
            "SessionInfoRegion::checked_range",
        )
    }

    pub fn bytes<'a>(&self, source: &'a [u8]) -> Result<&'a [u8]> {
        Ok(&source[self.checked_range(source.len())?])
    }

    pub fn buffer(&self, source: &[u8]) -> Result<SessionInfoBuffer> {
        Ok(SessionInfoBuffer::from_checked_region(self.bytes(source)?))
    }

    pub fn is_valid(&self) -> bool {
        self.offset > 0 && self.length > 0
    }
}

impl TryFrom<&Header> for SessionInfoRegion {
    type Error = IRacingSDKError;

    fn try_from(value: &Header) -> Result<Self> {
        Ok(SessionInfoRegion {
            offset: usize::try_from(value.session_info_offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "SessionInfoRegion::try_from",
                    format!("Could not convert {} to usize", value.session_info_offset),
                )
            })?,
            length: usize::try_from(value.session_info_len).map_err(|_| {
                IRacingSDKError::parse_error(
                    "SessionInfoRegion::try_from",
                    format!("Could not convert {} to usize", value.session_info_len),
                )
            })?,
        })
    }
}

/// Location and size of the variable-header region advertised by a [`Header`].
///
/// Construct with [`TryFrom<&Header>`](TryFrom::try_from). Construction validates
/// the offset, count, and byte-length calculation, but does not check that the
/// region fits in a source. Use [`Self::checked_range`], [`Self::bytes`], or
/// [`Self::buffer`] to validate source bounds before accessing the region.
/// Individual variable headers are not semantically validated by this type.
pub struct VariableHeaderRegion {
    offset: usize,
    length: usize,
    count: usize,
}

impl VariableHeaderRegion {
    /// Returns the region's byte offset from the beginning of the source.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the region's byte length: the header count times the wire size
    /// of a [`VariableHeader`].
    pub fn length(&self) -> usize {
        self.length
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
        checked_range(
            self.offset,
            self.length,
            data_len,
            "VariableHeaderRegion::checked_range",
        )
    }

    /// Borrows the region's bytes from the complete source without copying.
    ///
    /// `source` must begin at the origin used by [`Self::offset`].
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
    /// `source` must begin at the origin used by [`Self::offset`]. The returned
    /// buffer is independent of `source`; individual headers are not
    /// semantically validated.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the region's end offset overflows `usize` or
    /// the region extends beyond `source`.
    pub fn buffer(&self, source: &[u8]) -> Result<VariableHeadersBuffer> {
        Ok(VariableHeadersBuffer::from_checked_region(
            self.bytes(source)?,
        ))
    }

    pub fn is_valid(&self) -> bool {
        self.offset > 0 && self.length > 0
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
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "VariableHeaderRegion::try_from",
                    "Variable headers size calculation overflowed".to_string(),
                )
            })?;

        Ok(VariableHeaderRegion {
            offset,
            length,
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionInfoRegion, VariableHeaderRegion, checked_range};
    use crate::irsdk::{Header, VariableHeader, WireType};

    #[test]
    fn session_region_rejects_negative_header_fields() {
        for (offset, length) in [(-1, 1), (1, -1)] {
            let mut header = Header::read_from_bytes(&[0; Header::WIRE_SIZE]).unwrap();
            header.session_info_offset = offset;
            header.session_info_len = length;
            assert!(SessionInfoRegion::try_from(&header).is_err());
        }
    }

    #[test]
    fn session_region_copies_only_advertised_bytes() {
        let mut header = Header::read_from_bytes(&[0; Header::WIRE_SIZE]).unwrap();
        header.session_info_offset = 4;
        header.session_info_len = 7;
        let region = SessionInfoRegion::try_from(&header).unwrap();
        let mut source = b"skipSessionpadding".to_vec();
        assert_eq!(region.bytes(&source).unwrap(), b"Session");
        let buffer = region.buffer(&source).unwrap();
        source.fill(0);
        assert_eq!(String::from(buffer), "Session");
    }

    #[test]
    fn session_region_requires_the_full_region_even_with_early_nul() {
        let region = SessionInfoRegion {
            offset: 1,
            length: 4,
        };
        assert!(region.buffer(b"x\0").is_err());
        assert!(region.buffer(b"x\0pad").is_ok());
    }

    #[test]
    fn variable_header_region_rejects_negative_count() {
        let mut header = Header::read_from_bytes(&[0; Header::WIRE_SIZE]).unwrap();
        header.variable_count = -1;

        assert!(VariableHeaderRegion::try_from(&header).is_err());
    }

    #[test]
    fn variable_header_region_rejects_negative_offset() {
        let mut header = Header::read_from_bytes(&[0; Header::WIRE_SIZE]).unwrap();
        header.variable_header_offset = -1;

        assert!(VariableHeaderRegion::try_from(&header).is_err());
    }

    #[test]
    fn checked_range_accepts_region_within_data() {
        assert_eq!(checked_range(4, 6, 10, "test").unwrap(), 4..10);
    }

    #[test]
    fn checked_range_rejects_region_beyond_data() {
        assert!(checked_range(4, 7, 10, "test").is_err());
    }

    #[test]
    fn checked_range_rejects_endpoint_overflow() {
        assert!(checked_range(usize::MAX, 1, usize::MAX, "test").is_err());
    }

    #[test]
    fn variable_header_region_extracts_bytes_and_owned_buffer() {
        let region = VariableHeaderRegion {
            offset: 4,
            length: VariableHeader::WIRE_SIZE,
            count: 1,
        };
        let source = vec![0; 4 + VariableHeader::WIRE_SIZE];

        assert_eq!(
            region.bytes(&source).unwrap().len(),
            VariableHeader::WIRE_SIZE
        );
        assert_eq!(region.buffer(&source).unwrap().iter_headers().len(), 1);
    }

    #[test]
    fn variable_header_region_rejects_short_source() {
        let region = VariableHeaderRegion {
            offset: 4,
            length: VariableHeader::WIRE_SIZE,
            count: 1,
        };

        assert!(region.bytes(&[0; 4]).is_err());
        assert!(region.buffer(&[0; 4]).is_err());
    }

    // #[test]
    // fn session_info_region_extracts_bytes_and_owned_buffer() {
    //     let region = SessionInfoRegion {
    //         offset: 4,
    //         length: 7,
    //     };
    //     let source = b"skipSession";

    //     assert_eq!(region.bytes(source).unwrap(), b"Session");
    //     let session: String = region.buffer(source).unwrap().into();
    //     assert_eq!(session, "Session");
    // }

    // #[test]
    // fn session_info_region_rejects_short_source() {
    //     let region = SessionInfoRegion {
    //         offset: 4,
    //         length: 7,
    //     };

    //     assert!(region.bytes(b"short").is_err());
    //     assert!(region.buffer(b"short").is_err());
    // }
}
