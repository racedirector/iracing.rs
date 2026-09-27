use std::num::NonZeroUsize;

use iracing_irsdk::{Header, constants::IRSDK_VER as IRSDK_VERSION};
use zerocopy::IntoBytes;

use super::IBT_PREAMBLE_SIZE;
use crate::{ByteRegion, FrameRegion, FramesRegion, IRacingSDKError, MetadataRegions, Result};

/// Parsed IBT header fields used to derive a byte layout.
///
/// This type converts raw iRacing SDK header fields into domain types before
/// layout construction. Once constructed, offsets and lengths are representable
/// as `usize`, the tick rate and frame size are nonzero, and the SDK version has
/// been accepted.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ParsedIbtHeader {
    session_info_offset: usize,
    session_info_length: usize,
    variable_header_offset: usize,
    variable_count: usize,
    frame_size: NonZeroUsize,
}

impl ParsedIbtHeader {
    /// Parses the raw SDK header fields needed by an IBT byte layout.
    ///
    /// # Errors
    ///
    /// Returns a parse error when the header is blank, uses an unsupported SDK
    /// version, advertises a non-positive tick rate or frame size, or contains
    /// a negative offset, length, or count.
    pub(crate) fn try_from_header(header: &Header) -> Result<Self> {
        if header.as_bytes().iter().all(|&byte| byte == 0) {
            return Err(IRacingSDKError::parse_error(
                "ParsedIbtHeader::try_from_header",
                "Header is all zeros",
            ));
        }

        if header.version != IRSDK_VERSION {
            return Err(IRacingSDKError::parse_error(
                "ParsedIbtHeader::try_from_header",
                format!("Expected version {}, got {}", IRSDK_VERSION, header.version),
            ));
        }

        parse_positive_usize(
            "tick_rate",
            header.tick_rate,
            "ParsedIbtHeader::try_from_header",
        )?;

        let session_info_offset = parse_nonnegative_usize(
            "session_info_offset",
            header.session_info_offset,
            "ParsedIbtHeader::try_from_header",
        )?;
        let session_info_length = parse_nonnegative_usize(
            "session_info_length",
            header.session_info_length,
            "ParsedIbtHeader::try_from_header",
        )?;
        let variable_header_offset = parse_nonnegative_usize(
            "variable_header_offset",
            header.variable_header_offset,
            "ParsedIbtHeader::try_from_header",
        )?;
        let variable_count = parse_nonnegative_usize(
            "variable_count",
            header.variable_count,
            "ParsedIbtHeader::try_from_header",
        )?;
        let frame_size = parse_positive_usize(
            "buffer_length",
            header.buffer_length,
            "ParsedIbtHeader::try_from_header",
        )?;

        Ok(Self {
            session_info_offset,
            session_info_length,
            variable_header_offset,
            variable_count,
            frame_size,
        })
    }

    /// Returns the parsed session-info offset.
    pub fn session_info_offset(&self) -> usize {
        self.session_info_offset
    }

    /// Returns the parsed session-info length.
    pub fn session_info_length(&self) -> usize {
        self.session_info_length
    }

    /// Returns the parsed variable-header offset.
    pub fn variable_header_offset(&self) -> usize {
        self.variable_header_offset
    }

    /// Returns the parsed variable count.
    pub fn variable_count(&self) -> usize {
        self.variable_count
    }

    /// Returns the parsed telemetry frame size.
    pub fn frame_size(&self) -> NonZeroUsize {
        self.frame_size
    }
}

impl TryFrom<&Header> for ParsedIbtHeader {
    type Error = IRacingSDKError;

    fn try_from(header: &Header) -> Result<Self> {
        Self::try_from_header(header)
    }
}

fn parse_nonnegative_usize(
    field_name: &'static str,
    value: i32,
    context: &'static str,
) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        IRacingSDKError::parse_error(
            context,
            format!("{field_name} must be nonnegative, got {value}"),
        )
    })
}

fn parse_positive_usize(
    field_name: &'static str,
    value: i32,
    context: &'static str,
) -> Result<NonZeroUsize> {
    let value = parse_nonnegative_usize(field_name, value, context)?;
    NonZeroUsize::new(value).ok_or_else(|| {
        IRacingSDKError::parse_error(context, format!("{field_name} must be positive, got 0"))
    })
}

/// Validated locations of the metadata and telemetry frames in an IBT source.
///
/// The layout is derived from an iRacing SDK [`Header`] and the total source
/// length. It records byte locations only; it does not borrow or parse the
/// bytes in those locations.
#[derive(Debug, Clone)]
pub struct IbtLayout {
    metadata: MetadataRegions,
    frames: FramesRegion,
}

impl IbtLayout {
    /// Builds and validates the byte layout advertised by an IBT header.
    ///
    /// `source_len` is the length of the complete IBT source, including its
    /// fixed preamble, metadata, and telemetry frames.
    ///
    /// # Errors
    ///
    /// Returns a parse error when the header is invalid, a metadata region is
    /// outside the source or overlaps another metadata region, or the remaining
    /// frame bytes cannot be divided into complete frames of the advertised size.
    pub fn try_from_headers(header: &Header, source_len: usize) -> Result<Self> {
        if source_len < IBT_PREAMBLE_SIZE {
            return Err(IRacingSDKError::parse_error(
                "IbtLayout::try_from_headers",
                format!(
                    "source length {source_len} is shorter than IBT preamble ({IBT_PREAMBLE_SIZE})"
                ),
            ));
        }

        let parsed_header = ParsedIbtHeader::try_from_header(header)?;

        let metadata = MetadataRegions::try_from_parsed_header(&parsed_header, source_len)?;

        let frame_data_start = metadata.end();

        if frame_data_start > source_len {
            return Err(IRacingSDKError::parse_error(
                "IbtLayout::try_from_headers",
                "Frame data is out of range",
            ));
        }

        let frames = FramesRegion::new(
            ByteRegion::try_from(frame_data_start..source_len)?,
            parsed_header.frame_size().get(),
        )?;

        Ok(Self { metadata, frames })
    }

    /// Returns the validated variable-header and session-information regions.
    pub fn metadata(&self) -> &MetadataRegions {
        &self.metadata
    }

    /// Returns the region containing all recorded telemetry frames.
    pub fn frames(&self) -> &FramesRegion {
        &self.frames
    }

    /// Returns the byte region occupied by the frame at `index`.
    ///
    /// # Errors
    ///
    /// Returns a parse error if `index` is outside the recorded frame range or
    /// if calculating the frame offset overflows `usize`.
    pub fn frame(&self, index: usize) -> Result<FrameRegion> {
        self.frames.frame(index)
    }

    /// Returns the number of complete telemetry frames in the source.
    pub fn frame_count(&self) -> usize {
        self.frames.frame_count()
    }

    /// Returns the size of one telemetry frame in bytes.
    pub fn frame_size(&self) -> usize {
        self.frames.frame_size()
    }

    /// Returns the source-relative byte offset at which telemetry frames begin.
    pub fn frame_data_start(&self) -> usize {
        self.frames.as_region().offset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_irsdk::VariableHeader;
    use zerocopy::FromZeros;

    // Only byte geometry is modeled here; no telemetry values are decoded.
    fn valid_header() -> Header {
        let mut header = Header::new_zeroed();
        header.version = IRSDK_VERSION;
        header.tick_rate = 60;
        header.buffer_length = 4;
        header
    }

    #[test]
    fn rejects_truncated_preamble_and_blank_header() {
        for source_len in [0, IBT_PREAMBLE_SIZE - 1] {
            assert!(IbtLayout::try_from_headers(&valid_header(), source_len).is_err());
        }
        assert!(IbtLayout::try_from_headers(&Header::new_zeroed(), IBT_PREAMBLE_SIZE).is_err());
    }

    #[test]
    fn rejects_invalid_header_fields() {
        type InvalidHeaderCase = (&'static str, fn(&mut Header));
        let cases: [InvalidHeaderCase; 10] = [
            ("version", |h| h.version = IRSDK_VERSION + 1),
            ("zero tick rate", |h| h.tick_rate = 0),
            ("negative tick rate", |h| h.tick_rate = -1),
            ("session offset", |h| h.session_info_offset = -1),
            ("session length", |h| h.session_info_length = -1),
            ("variable offset", |h| h.variable_header_offset = -1),
            ("variable count", |h| h.variable_count = -1),
            ("zero frame size", |h| h.buffer_length = 0),
            ("negative frame size", |h| h.buffer_length = -1),
            ("unsupported version", |h| h.version = -1),
        ];
        for (name, invalidate) in cases {
            let mut header = valid_header();
            invalidate(&mut header);
            assert!(
                IbtLayout::try_from_headers(&header, IBT_PREAMBLE_SIZE).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_invalid_metadata_geometry() {
        let preamble = i32::try_from(IBT_PREAMBLE_SIZE).unwrap();
        let variable_len = i32::try_from(size_of::<VariableHeader>()).unwrap();
        for (variable_offset, variable_count, session_offset, session_length, source_len) in [
            (
                preamble - 1,
                1,
                0,
                0,
                IBT_PREAMBLE_SIZE + size_of::<VariableHeader>(),
            ),
            (0, 0, preamble - 1, 1, IBT_PREAMBLE_SIZE),
            (
                preamble,
                1,
                0,
                0,
                IBT_PREAMBLE_SIZE + size_of::<VariableHeader>() - 1,
            ),
            (0, 0, preamble, 2, IBT_PREAMBLE_SIZE + 1),
            (
                preamble,
                1,
                preamble + 1,
                1,
                IBT_PREAMBLE_SIZE + size_of::<VariableHeader>(),
            ),
            (
                preamble,
                1,
                preamble,
                variable_len,
                IBT_PREAMBLE_SIZE + size_of::<VariableHeader>(),
            ),
        ] {
            let mut header = valid_header();
            header.variable_header_offset = variable_offset;
            header.variable_count = variable_count;
            header.session_info_offset = session_offset;
            header.session_info_length = session_length;
            assert!(IbtLayout::try_from_headers(&header, source_len).is_err());
        }
    }

    #[test]
    fn metadata_order_does_not_change_frame_geometry() {
        let variable_len = size_of::<VariableHeader>();
        let session_len = 7;
        let metadata_end = IBT_PREAMBLE_SIZE + variable_len + session_len;
        for (variable_offset, session_offset) in [
            (IBT_PREAMBLE_SIZE, IBT_PREAMBLE_SIZE + variable_len),
            (IBT_PREAMBLE_SIZE + session_len, IBT_PREAMBLE_SIZE),
        ] {
            let mut header = valid_header();
            header.variable_header_offset = i32::try_from(variable_offset).unwrap();
            header.variable_count = 1;
            header.session_info_offset = i32::try_from(session_offset).unwrap();
            header.session_info_length = i32::try_from(session_len).unwrap();
            let empty = IbtLayout::try_from_headers(&header, metadata_end).unwrap();
            assert_eq!(empty.frame_count(), 0);
            assert!(empty.frames().is_empty());
            assert!(empty.frame(0).is_err());

            let layout = IbtLayout::try_from_headers(&header, metadata_end + 12).unwrap();
            assert_eq!(layout.metadata().end(), metadata_end);
            assert_eq!(
                layout.metadata().variable_headers().unwrap().offset(),
                variable_offset
            );
            assert_eq!(
                layout.metadata().session_info().unwrap().offset(),
                session_offset
            );
            assert_eq!(layout.frame_data_start(), metadata_end);
            assert_eq!(layout.frame_size(), 4);
            assert_eq!(layout.frame_count(), 3);
            assert_eq!(
                layout.frame(0).unwrap().as_region().as_range(),
                metadata_end..metadata_end + 4
            );
            assert_eq!(
                layout.frame(2).unwrap().as_region().as_range(),
                metadata_end + 8..metadata_end + 12
            );
            assert!(layout.frame(3).is_err());
            assert!(layout.frame(usize::MAX).is_err());
            assert!(IbtLayout::try_from_headers(&header, metadata_end + 13).is_err());
        }
    }

    #[test]
    fn absent_metadata_ignores_unused_nonnegative_offsets() {
        let mut header = valid_header();
        header.variable_header_offset = i32::MAX;
        header.session_info_offset = i32::MAX;
        let layout = IbtLayout::try_from_headers(&header, IBT_PREAMBLE_SIZE).unwrap();
        assert!(layout.metadata().variable_headers().is_none());
        assert!(layout.metadata().session_info().is_none());
        assert_eq!(layout.frame_data_start(), IBT_PREAMBLE_SIZE);
        assert_eq!(layout.frame_count(), 0);
    }
}
