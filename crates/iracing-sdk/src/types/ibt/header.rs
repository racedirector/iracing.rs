use std::num::NonZeroUsize;

use iracing_irsdk::{Header, constants::IRSDK_VER as IRSDK_VERSION};
use zerocopy::IntoBytes;

use crate::{
    IRacingSDKError, Result,
    parse_utils::{parse_nonnegative_usize, parse_positive_usize},
};

/// Parsed IBT header fields used to derive a byte layout.
///
/// This type converts raw iRacing SDK header fields into domain types before
/// layout construction. Once constructed, offsets and lengths are representable
/// as `usize`, the tick rate and frame size are nonzero, and the SDK version has
/// been accepted.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ParsedIbtHeader {
    tick_rate: NonZeroUsize,
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

        let tick_rate = parse_positive_usize(
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
            tick_rate,
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

    /// Returns the parsed tick rate
    pub fn tick_rate(&self) -> NonZeroUsize {
        self.tick_rate
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
