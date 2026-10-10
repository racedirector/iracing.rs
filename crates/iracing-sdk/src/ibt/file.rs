//! Complete immutable IBT recording ownership.

use std::{fmt, fs::File, path::Path, sync::Arc};

use iracing_irsdk::{DiskSubHeader, Header, IbtHeader};
use memmap2::Mmap;

use crate::{
    IRacingSDKError, IbtLayout, Result, SessionInfoBytes, TelemetryLayout, VariableHeaders,
    source::ibt::Source,
};

/// One complete immutable `.ibt` recording and its validated metadata.
///
/// Construction snapshots the variable headers and session-information bytes
/// once and builds one shared telemetry layout. [`IbtLayout`] describes physical
/// file geometry; [`TelemetryLayout`] describes fields within a frame. Neither
/// metadata inspection nor construction reads or copies the complete frame region.
///
/// Recorded coordinates and source geometry use `usize`. Files too large for
/// the platform's address space cannot be mapped.
pub struct IbtFile {
    source: Source,
    header: IbtHeader,
    physical_layout: IbtLayout,
    variable_headers: VariableHeaders,
    session_info: Option<SessionInfoBytes>,
    telemetry_layout: Arc<TelemetryLayout>,
}

impl fmt::Debug for IbtFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IbtFile")
            .field("source_len", &self.source.len())
            .field("physical_layout", &self.physical_layout)
            .field("variable_count", &self.variable_headers.len())
            .field("has_session_info", &self.session_info.is_some())
            .finish_non_exhaustive()
    }
}

impl IbtFile {
    /// Opens a completed recording through a read-only memory map.
    ///
    /// The backing file must remain unchanged and untruncated by every process
    /// for this object's lifetime. Open completed recordings whose storage you
    /// control, or use [`Self::from_bytes`] with an owned copy instead.
    ///
    /// # Errors
    /// Returns an error if opening, mapping, or validating the recording fails.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;
        // SAFETY: The documented contract requires completed, immutable storage
        // for the entire mapping lifetime. Mmap owns its mapping independently
        // of `file`, unmaps it on drop, and no mapped references escape IbtFile.
        let mapped = unsafe { Mmap::map(&file) }.map_err(|error| IRacingSDKError::File {
            path,
            source: std::io::Error::new(error.kind(), format!("Failed to map IBT source: {error}")),
        })?;
        Self::from_source(Source::Mapped(mapped))
    }

    /// Takes ownership of the complete recording bytes and validates them.
    ///
    /// Metadata is established once; frame storage remains in the owned source.
    /// Passing a `Vec<u8>` transfers its allocation without copying it.
    ///
    /// # Errors
    /// Rejects malformed or truncated geometry, invalid variable metadata, and
    /// recordings containing frames without variable headers. Zero-frame
    /// recordings may have absent metadata and an empty telemetry layout.
    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Self::from_source(Source::Owned(bytes.into()))
    }

    fn from_source(source: Source) -> Result<Self> {
        let preamble_len = size_of::<Header>() + size_of::<DiskSubHeader>();
        let bytes = source.get(0..preamble_len).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "IbtFile::from_source",
                "Source is shorter than the IBT preamble",
            )
        })?;
        let header = IbtHeader::try_from_bytes(bytes).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtFile::from_source",
                "Could not parse IBT header from source",
            )
        })?;
        let physical_layout = IbtLayout::try_from_headers(header.header(), source.len())?;
        let variable_headers = match physical_layout.metadata().variable_headers() {
            Some(region) => {
                let bytes = source.get(region.as_region().as_range()).ok_or_else(|| {
                    IRacingSDKError::parse_error(
                        "IbtFile::from_source",
                        "Could not get variable headers bytes from source",
                    )
                })?;
                VariableHeaders::try_from_bytes(bytes, region.count())?
            }
            None => VariableHeaders::default(),
        };
        let session_info = match physical_layout.metadata().session_info() {
            Some(region) => {
                let bytes = source.get(region.as_region().as_range()).ok_or_else(|| {
                    IRacingSDKError::parse_error(
                        "IbtFile::from_source",
                        "Could not get session info bytes from source",
                    )
                })?;
                Some(SessionInfoBytes::from_checked_region(bytes))
            }
            None => None,
        };
        if physical_layout.frame_count() > 0 && variable_headers.is_empty() {
            return Err(IRacingSDKError::parse_error(
                "IbtFile::from_source",
                "Telemetry frames require variable-header metadata",
            ));
        }
        let telemetry_layout = Arc::new(TelemetryLayout::try_from_headers(
            &variable_headers,
            physical_layout.frame_size(),
        )?);
        let disk_header = header.disk_header();
        // Record counts are advisory; only validated physical geometry defines EOF.
        if disk_header.record_count > 0
            && physical_layout.frame_count() > 0
            && usize::try_from(disk_header.record_count).ok() != Some(physical_layout.frame_count())
        {
            tracing::warn!(
                "Frame count mismatch: disk header reports {} records, calculated {} frames from file size",
                disk_header.record_count,
                physical_layout.frame_count()
            );
        }
        Ok(Self {
            source,
            header,
            physical_layout,
            variable_headers,
            session_info,
            telemetry_layout,
        })
    }

    /// Returns the immutable recording header.
    pub fn header(&self) -> &Header {
        self.header.header()
    }

    /// Returns the immutable disk sub-header. Its record count is advisory.
    pub fn disk_header(&self) -> &DiskSubHeader {
        self.header.disk_header()
    }

    /// Returns the validated physical geometry of this exact source.
    pub fn physical_layout(&self) -> &IbtLayout {
        &self.physical_layout
    }

    /// Returns the recording's single shared, validated telemetry layout.
    pub fn telemetry_layout(&self) -> &Arc<TelemetryLayout> {
        &self.telemetry_layout
    }

    /// Borrows the owned variable-header snapshot in publication order.
    /// Absent metadata is represented by an empty snapshot.
    pub fn variable_headers(&self) -> &VariableHeaders {
        &self.variable_headers
    }

    /// Borrows the exact session-information snapshot, including its padding.
    /// Decoding and YAML parsing are separate operations on the snapshot.
    pub fn session_info_bytes(&self) -> Option<&SessionInfoBytes> {
        self.session_info.as_ref()
    }

    /// Returns the size in bytes of one telemetry frame.
    pub fn frame_size(&self) -> usize {
        self.physical_layout.frame_size()
    }

    /// Returns the total number of complete physical frame records.
    pub fn frame_count(&self) -> usize {
        self.physical_layout.frame_count()
    }

    /// Returns the recorded tick rate unchanged, without a playback policy.
    pub fn tick_rate(&self) -> i32 {
        self.header().tick_rate
    }
}

#[cfg(test)]
mod tests;
