use iracing_irsdk::{DiskSubHeader, Header};

use super::{IBT_PREAMBLE_SIZE, ParsedIbtHeader};
use crate::{ByteRegion, FrameRegion, FramesRegion, IRacingSDKError, MetadataRegions, Result};

/// Canonical physical layout description of a validated IBT source.
///
/// The layout is derived from an iRacing SDK [`Header`] and the total source
/// length. It records byte locations only; it does not borrow or parse the
/// bytes in those locations. Fixed header regions, optional metadata regions,
/// and EOF-delimited frame geometry are available without re-reading the header.
/// Both [`crate::ibt::IbtReader`] and inspection tools can use this description;
/// it is independent of replay cursors, telemetry field layouts, and formatting.
/// All byte coordinates are source-relative and fit within [`Self::source_len`].
///
/// Frame start is the greatest present metadata endpoint (or the preamble end
/// when metadata is absent). Frame count comes from physical EOF and frame size,
/// not the disk sub-header's advisory record count.
///
/// # Inspection
///
/// ```no_run
/// use iracing_sdk::ibt::IbtReader;
///
/// # fn inspect() -> iracing_sdk::Result<()> {
/// let reader = IbtReader::open("telemetry.ibt")?;
/// let layout = reader.layout();
/// println!("source: {} bytes", layout.source_len());
/// println!("header: {:?}", layout.header_region().as_range());
/// println!("disk sub-header: {:?}", layout.disk_header_region().as_range());
/// if let Some(region) = layout.metadata().variable_headers() {
///     println!("variable headers: {:?}", region.as_region().as_range());
/// }
/// if let Some(region) = layout.metadata().session_info() {
///     println!("session info: {:?}", region.as_region().as_range());
/// }
/// println!("frames: {:?}", layout.frames().as_region().as_range());
/// println!("{} frames of {} bytes", layout.frame_count(), layout.frame_size());
/// # Ok(())
/// # }
/// ```
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

    /// Returns the total source length in bytes, including the preamble.
    pub fn source_len(&self) -> usize {
        // The validated frame region extends to physical EOF, even when empty.
        self.frames.end()
    }

    /// Returns the fixed main-header region at the beginning of the source.
    pub fn header_region(&self) -> ByteRegion {
        ByteRegion::new(0, size_of::<Header>()).expect("fixed header region fits usize")
    }

    /// Returns the fixed disk sub-header region immediately after the main header.
    pub fn disk_header_region(&self) -> ByteRegion {
        ByteRegion::new(size_of::<Header>(), size_of::<DiskSubHeader>())
            .expect("fixed IBT preamble fits usize")
    }

    /// Returns the complete fixed preamble containing both headers.
    pub fn preamble_region(&self) -> ByteRegion {
        ByteRegion::new(0, IBT_PREAMBLE_SIZE).expect("fixed IBT preamble fits usize")
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
        self.frames.start()
    }
}

#[cfg(test)]
mod tests {
    use iracing_irsdk::{VariableHeader, constants::IRSDK_VER as IRSDK_VERSION};
    use zerocopy::FromZeros;

    use super::*;

    // Only byte geometry is modeled here; no telemetry values are decoded.
    fn valid_header() -> Header {
        let mut header = Header::new_zeroed();
        header.version = IRSDK_VERSION;
        header.tick_rate = 60;
        header.buffer_length = 4;
        header
    }

    #[test]
    fn fixed_regions_are_contiguous_and_source_bounded() {
        // SDK wire sizes: the combined preamble must not be confused with Header.
        for source_len in [144, 156] {
            let layout = IbtLayout::try_from_headers(&valid_header(), source_len).unwrap();
            assert_eq!(layout.source_len(), source_len);
            assert_eq!(layout.header_region().as_range(), 0..112);
            assert_eq!(layout.disk_header_region().as_range(), 112..144);
            assert_eq!(layout.preamble_region().as_range(), 0..144);
            assert_eq!(
                layout.header_region().end(),
                layout.disk_header_region().offset()
            );
            assert_eq!(
                layout.disk_header_region().end(),
                layout.preamble_region().end()
            );
            assert!(layout.preamble_region().end() <= layout.source_len());
            assert_eq!(layout.frames().as_region().as_range(), 144..source_len);
            assert_eq!(layout.frame_count(), (source_len - 144) / 4);
        }
    }

    #[test]
    fn inspection_preserves_gaps_and_single_metadata_regions() {
        let variable_len = size_of::<VariableHeader>();
        let variable_offset = IBT_PREAMBLE_SIZE + 11;
        let session_offset = variable_offset + variable_len + 13;
        for (has_variables, has_session) in [(true, false), (false, true), (true, true)] {
            let mut header = valid_header();
            if has_variables {
                header.variable_header_offset = i32::try_from(variable_offset).unwrap();
                header.variable_count = 1;
            }
            if has_session {
                header.session_info_offset = i32::try_from(session_offset).unwrap();
                header.session_info_length = 7;
            }
            let metadata_end = if has_session {
                session_offset + 7
            } else {
                variable_offset + variable_len
            };
            let layout = IbtLayout::try_from_headers(&header, metadata_end + 8).unwrap();
            assert_eq!(layout.source_len(), metadata_end + 8);
            assert_eq!(
                layout
                    .metadata()
                    .variable_headers()
                    .map(|r| r.as_region().as_range()),
                has_variables.then_some(variable_offset..variable_offset + variable_len)
            );
            assert_eq!(
                layout
                    .metadata()
                    .session_info()
                    .map(|r| r.as_region().as_range()),
                has_session.then_some(session_offset..session_offset + 7)
            );
            assert_eq!(layout.frame_data_start(), metadata_end);
            assert_eq!(
                layout.frames().as_region().as_range(),
                metadata_end..metadata_end + 8
            );
            assert_eq!(layout.frame(1).unwrap().end(), layout.source_len());
        }
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
