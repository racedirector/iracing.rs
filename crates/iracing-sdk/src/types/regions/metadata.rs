use crate::{
    IRacingSDKError, Result,
    irsdk::Header,
    parse_utils::validate_metadata_region,
    types::{
        SessionInfoRegion, VariableHeadersRegion,
        ibt::{IBT_PREAMBLE_SIZE, ParsedIbtHeader},
    },
};

/// Validated metadata regions that precede the telemetry frames in an IBT source.
///
/// Empty session-information and variable-header regions are represented by
/// `None`. The computed end always includes at least the fixed IBT preamble.
#[derive(Debug, Clone)]
pub struct MetadataRegions {
    variable_headers: Option<VariableHeadersRegion>,
    session_info: Option<SessionInfoRegion>,
    end: usize,
}

impl MetadataRegions {
    /// Derives and validates the metadata regions advertised by a parsed IBT header.
    ///
    /// `source_len` is the length of the complete IBT source. The individual
    /// metadata regions must begin after the fixed IBT preamble, fit within the
    /// source, and not overlap each other.
    ///
    /// # Errors
    ///
    /// Returns a parse error if a region-size calculation overflows, a region
    /// lies outside the valid metadata area, or the two regions overlap.
    pub(crate) fn try_from_parsed_header(
        header: &ParsedIbtHeader,
        source_len: usize,
    ) -> Result<Self> {
        let variable_headers = VariableHeadersRegion::try_from_parsed_header(header)?;
        let session_info = SessionInfoRegion::try_from_parsed_header(header)?;

        Self::from_regions(variable_headers, session_info, source_len)
    }

    /// Derives and validates the metadata regions advertised by `header`.
    ///
    /// `source_len` is the length of the complete IBT source. The individual
    /// metadata regions must begin after the fixed IBT preamble, fit within the
    /// source, and not overlap each other.
    ///
    /// # Errors
    ///
    /// Returns a parse error if a header field cannot be represented safely,
    /// the source is shorter than the preamble, a region lies outside the valid
    /// metadata area, or the two regions overlap.
    pub fn try_from_header(header: &Header, source_len: usize) -> Result<Self> {
        // Construct and validate variable headers region
        let variable_headers = VariableHeadersRegion::try_from_header(header)?;

        // Construct and validate session info region
        let session_info = SessionInfoRegion::try_from_header(header)?;

        Self::from_regions(variable_headers, session_info, source_len)
    }

    fn from_regions(
        variable_headers: Option<VariableHeadersRegion>,
        session_info: Option<SessionInfoRegion>,
        source_len: usize,
    ) -> Result<Self> {
        if source_len < IBT_PREAMBLE_SIZE {
            return Err(IRacingSDKError::parse_error(
                "MetadataRegions",
                format!(
                    "source length {source_len} is shorter than IBT preamble ({IBT_PREAMBLE_SIZE})"
                ),
            ));
        }

        let source_len_u64 = u64::try_from(source_len).map_err(|_| {
            IRacingSDKError::parse_error(
                "MetadataRegions",
                "source length cannot be represented as a source offset",
            )
        })?;
        let preamble_end = u64::try_from(IBT_PREAMBLE_SIZE).map_err(|_| {
            IRacingSDKError::parse_error(
                "MetadataRegions",
                "IBT preamble size cannot be represented as a source offset",
            )
        })?;

        if let Some(region) = variable_headers {
            validate_metadata_region(
                "VariableHeaders",
                region.as_region(),
                source_len_u64,
                preamble_end,
            )?;
        }

        if let Some(region) = session_info {
            validate_metadata_region(
                "SessionInfo",
                region.as_region(),
                source_len_u64,
                preamble_end,
            )?;
        }

        // Ensure the regions don't overlap
        if let (Some(variables), Some(session)) = (variable_headers, session_info)
            && variables.overlaps(session.as_region())
        {
            return Err(IRacingSDKError::parse_error(
                "MetadataRegions",
                "metadata regions overlap",
            ));
        }

        // Compute the end of the metadata region
        let end = [
            variable_headers.map(|r| r.end()),
            session_info.map(|r| r.end()),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(IBT_PREAMBLE_SIZE);

        Ok(Self {
            variable_headers,
            session_info,
            end,
        })
    }

    /// Returns the variable-header region, or `None` when no headers were advertised.
    pub fn variable_headers(&self) -> Option<&VariableHeadersRegion> {
        self.variable_headers.as_ref()
    }

    /// Returns the session-information region, or `None` when it was empty.
    pub fn session_info(&self) -> Option<&SessionInfoRegion> {
        self.session_info.as_ref()
    }

    /// Returns the first byte offset after all metadata and the fixed preamble.
    pub fn end(&self) -> usize {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use iracing_irsdk::VariableHeader;
    use zerocopy::FromZeros;

    use super::*;

    fn empty_header() -> Header {
        Header::new_zeroed()
    }

    fn as_i32(value: usize) -> i32 {
        i32::try_from(value).expect("test offset fits in i32")
    }

    fn variable_header_len(count: usize) -> usize {
        count * size_of::<VariableHeader>()
    }

    #[test]
    fn absent_metadata_still_requires_complete_preamble() {
        for source_len in [0, IBT_PREAMBLE_SIZE - 1] {
            assert!(MetadataRegions::try_from_header(&empty_header(), source_len).is_err());
        }
        let metadata =
            MetadataRegions::try_from_header(&empty_header(), IBT_PREAMBLE_SIZE).unwrap();
        assert_eq!(metadata.end(), IBT_PREAMBLE_SIZE);
        assert!(metadata.variable_headers().is_none());
        assert!(metadata.session_info().is_none());
    }

    #[test]
    fn metadata_rejects_variable_headers_before_preamble() {
        let preamble = IBT_PREAMBLE_SIZE;
        let mut header = empty_header();
        header.variable_count = 1;
        header.variable_header_offset = as_i32(preamble - 1);

        assert!(
            MetadataRegions::try_from_header(&header, preamble + variable_header_len(1)).is_err()
        );
    }

    #[test]
    fn metadata_rejects_session_info_before_preamble() {
        let preamble = IBT_PREAMBLE_SIZE;
        let mut header = empty_header();
        header.session_info_offset = as_i32(preamble - 1);
        header.session_info_length = 1;

        assert!(MetadataRegions::try_from_header(&header, preamble).is_err());
    }

    #[test]
    fn metadata_rejects_variable_headers_beyond_source_len() {
        let preamble = IBT_PREAMBLE_SIZE;
        let mut header = empty_header();
        header.variable_count = 1;
        header.variable_header_offset = as_i32(preamble);

        assert!(
            MetadataRegions::try_from_header(&header, preamble + variable_header_len(1) - 1)
                .is_err()
        );
    }

    #[test]
    fn metadata_rejects_session_info_beyond_source_len() {
        let preamble = IBT_PREAMBLE_SIZE;
        let mut header = empty_header();
        header.session_info_offset = as_i32(preamble);
        header.session_info_length = 2;

        assert!(MetadataRegions::try_from_header(&header, preamble + 1).is_err());
    }

    #[test]
    fn metadata_rejects_overlapping_variable_headers_and_session_info() {
        let preamble = IBT_PREAMBLE_SIZE;
        let mut header = empty_header();
        header.variable_count = 1;
        header.variable_header_offset = as_i32(preamble);
        header.session_info_offset = as_i32(preamble + 1);
        header.session_info_length = 1;

        assert!(
            MetadataRegions::try_from_header(&header, preamble + variable_header_len(1)).is_err()
        );
    }

    #[test]
    fn metadata_end_is_greatest_valid_metadata_end() {
        let preamble = IBT_PREAMBLE_SIZE;
        let variable_len = variable_header_len(1);

        let cases = [
            (0, 0, 0, 0, preamble),
            (preamble, 1, 0, 0, preamble + variable_len),
            (0, 0, preamble + 11, 7, preamble + 18),
            (
                preamble,
                1,
                preamble + variable_len,
                23,
                preamble + variable_len + 23,
            ),
            (preamble + 4, 1, preamble, 4, preamble + 4 + variable_len),
        ];

        for (variable_offset, variable_count, session_offset, session_len, expected_end) in cases {
            let mut header = empty_header();
            header.variable_header_offset = as_i32(variable_offset);
            header.variable_count = variable_count;
            header.session_info_offset = as_i32(session_offset);
            header.session_info_length = as_i32(session_len);

            let metadata = MetadataRegions::try_from_header(&header, expected_end).unwrap();

            assert_eq!(metadata.end(), expected_end);
        }
    }
}
