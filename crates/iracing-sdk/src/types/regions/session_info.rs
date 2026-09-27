use super::ByteRegion;
use crate::{IRacingSDKError, Result, irsdk::Header};

/// Location and size of the session-information bytes advertised by an SDK header.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionInfoRegion {
    region: ByteRegion,
}

impl SessionInfoRegion {
    /// Derives the session-information region advertised by an SDK header.
    ///
    /// Returns `Ok(None)` when the header advertises a zero-length region.
    /// Source bounds are not checked; callers that have a complete source
    /// should compare [`Self::end`] with its length before slicing.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the advertised offset or length cannot be
    /// represented as `usize`, or if their sum overflows `usize`.
    pub fn try_from_header(header: &Header) -> Result<Option<Self>> {
        let offset = usize::try_from(header.session_info_offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "SessionInfoRegion::try_from_header",
                format!("Could not convert {} to usize", header.session_info_offset),
            )
        })?;

        let length = usize::try_from(header.session_info_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "SessionInfoRegion::try_from_header",
                format!("Could not convert {} to usize", header.session_info_length),
            )
        })?;

        if length == 0 {
            return Ok(None);
        }

        Ok(Some(Self {
            region: ByteRegion::new(offset, length)?,
        }))
    }

    /// Returns the associated byte region.
    pub fn as_region(&self) -> ByteRegion {
        self.region
    }

    /// Returns the source-relative starting byte offset.
    pub fn offset(self) -> usize {
        self.region.offset()
    }

    /// Returns the length of the session-information region in bytes.
    pub fn len(self) -> usize {
        self.region.len()
    }

    /// Returns whether the session-information region contains no bytes.
    pub fn is_empty(self) -> bool {
        self.region.is_empty()
    }

    /// Returns the exclusive source-relative end offset.
    pub fn end(&self) -> usize {
        self.region.end()
    }

    /// Returns whether each region begins before the other region ends.
    pub fn overlaps(&self, other: ByteRegion) -> bool {
        self.region.overlaps(other)
    }
}

impl TryFrom<&Header> for SessionInfoRegion {
    type Error = IRacingSDKError;

    /// Derives a nonempty session-information region from an SDK header.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the header fields cannot be represented safely
    /// or if the header advertises an empty session-information region.
    fn try_from(value: &Header) -> Result<Self> {
        match Self::try_from_header(value)? {
            Some(region) => Ok(region),
            None => Err(IRacingSDKError::parse_error(
                "SessionInfoRegion::try_from",
                "Header advertised empty session info region",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::FromZeros;

    #[test]
    fn session_region_rejects_negative_header_fields() {
        for (offset, length) in [(-1, 1), (1, -1)] {
            let mut header = Header::new_zeroed();
            header.session_info_offset = offset;
            header.session_info_length = length;

            assert!(SessionInfoRegion::try_from_header(&header).is_err());
        }
    }

    #[test]
    fn session_region_empty_none() {
        let header = Header::new_zeroed();

        assert!(
            SessionInfoRegion::try_from_header(&header)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn session_region_valid() {
        let mut header = Header::new_zeroed();
        header.session_info_length = 10;

        assert!(
            SessionInfoRegion::try_from_header(&header)
                .unwrap()
                .is_some()
        );
    }
}
