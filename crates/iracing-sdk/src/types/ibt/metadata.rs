use iracing_irsdk::DiskSubHeader;

use crate::{
    IRacingSDKError, Result,
    irsdk::Header,
    types::{ByteRegion, SessionInfoRegion, VariableHeadersRegion},
};

fn validate_metadata_region(
    name: &'static str,
    region: ByteRegion,
    source_len: usize,
) -> Result<()> {
    ensure!(
        region.offset() >= MetadataRegions::IBT_PREAMBLE_SIZE,
        IRacingSDKError::parse_error(
            "IBT metadata layout",
            format!(
                "{name} starts at {}, before preamble end {}",
                region.offset(),
                MetadataRegions::IBT_PREAMBLE_SIZE
            ),
        )
    );

    ensure!(
        region.end() <= source_len,
        IRacingSDKError::parse_error(
            "IBT metadata layout",
            format!(
                "{name} ends at {}, beyond source length {source_len}",
                region.end(),
            ),
        )
    );

    Ok(())
}

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
    const IBT_PREAMBLE_SIZE: usize = size_of::<Header>() + size_of::<DiskSubHeader>();

    /// Derives and validates the metadata regions advertised by `header`.
    ///
    /// `source_len` is the length of the complete IBT source. The individual
    /// metadata regions must begin after the fixed IBT preamble, fit within the
    /// source, and not overlap each other.
    ///
    /// # Errors
    ///
    /// Returns a parse error if a header field cannot be represented safely,
    /// a region lies outside the valid metadata area, or the two regions overlap.
    pub fn try_from_header(header: &Header, source_len: usize) -> Result<Self> {
        // Construct and validate variable headers region
        let variable_headers = VariableHeadersRegion::try_from_header(header)?;
        if let Some(region) = variable_headers {
            validate_metadata_region("VariableHeaders", region.as_region(), source_len)?;
        }

        // Construct and validate session info region
        let session_info = SessionInfoRegion::try_from_header(header)?;
        if let Some(region) = session_info {
            validate_metadata_region("SessionInfo", region.as_region(), source_len)?;
        }

        // Ensure the regions don't overlap
        if let (Some(variables), Some(session)) = (variable_headers, session_info) {
            ensure!(
                !variables.overlaps(session.as_region()),
                IRacingSDKError::parse_error(
                    "MetadataRegions::try_from_header",
                    "metadata regions overlap"
                )
            )
        }

        // Compute the end of the metadata region
        let end = [
            variable_headers.map(|r| r.end()),
            session_info.map(|r| r.end()),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(Self::IBT_PREAMBLE_SIZE);

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
