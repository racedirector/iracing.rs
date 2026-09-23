use iracing_irsdk::{Header, constants::IRSDK_VER as IRSDK_VERSION};
use zerocopy::IntoBytes;

use crate::{
    ByteRegion, FrameRegion, FramesRegion, IRacingSDKError, Result,
    types::ibt::metadata::MetadataRegions,
};

/// Validated locations of the metadata and telemetry frames in an IBT source.
///
/// The layout is derived from an iRacing SDK [`Header`] and the total source
/// length. It records byte locations only; it does not borrow or parse the
/// bytes in those locations.
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
        ensure!(
            source_len >= 144_usize,
            IRacingSDKError::parse_error(
                "IbtLayout::try_from_headers",
                format!("source length {source_len} is shorter than IBT preamble (144)")
            )
        );

        ensure!(
            !header.as_bytes().iter().all(|&byte| byte == 0),
            IRacingSDKError::parse_error("IbtLayout::try_from_headers", "Header is all zeros")
        );
        ensure_eq!(
            header,
            version,
            IRSDK_VERSION,
            "IbtLayout::try_from_headers"
        );
        ensure_positive!(header, tick_rate, "IbtLayout::try_from_headers");
        ensure_nonnegative!(header, session_info_offset, "IbtLayout::try_from_headers");
        ensure_nonnegative!(header, session_info_length, "IbtLayout::try_from_headers");
        ensure_nonnegative!(
            header,
            variable_header_offset,
            "IbtLayout::try_from_headers"
        );
        ensure_nonnegative!(header, variable_count, "IbtLayout::try_from_headers");

        let metadata = MetadataRegions::try_from_header(header, source_len)?;

        let frame_data_start = metadata.end();

        ensure!(
            frame_data_start <= source_len,
            IRacingSDKError::parse_error("IbtHeader::try_from", "Frame data is out of range")
        );

        let frames = FramesRegion::new(
            ByteRegion::try_from(frame_data_start..source_len)?,
            usize::try_from(header.buffer_length).map_err(|_| {
                IRacingSDKError::parse_error(
                    "IbtLayout::try_from_headers",
                    "Could not parse frame size into `usize`",
                )
            })?,
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
