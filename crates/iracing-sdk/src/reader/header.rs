//! Header-directed interpretation and extraction of SDK byte regions.
//!
//! [`HeaderRegions`] converts the signed offsets, counts, and lengths in one
//! [`Header`] snapshot into checked Rust values. [`HeaderSnapshotReader`] then
//! combines that immutable interpretation with a [`RandomAccessSource`] to
//! produce owned SDK buffer newtypes.
//!
//! The split between interpretation and extraction is deliberate:
//!
//! - region construction checks signed conversions and arithmetic overflow;
//! - source extraction checks that each region fits within the actual source;
//! - source-specific readers remain responsible for validating the complete
//!   header and for live-memory consistency checks.
//!
//! This module does not call [`Header::validate`](crate::types::Header::validate)
//! and does not assert that independently copied live regions came from one
//! atomic simulator state.
//!
//! # Why separate layout from extraction?
//!
//! Header interpretation can fail because signed SDK fields are invalid or
//! arithmetic overflows; extraction can fail because an otherwise representable
//! region exceeds the actual source. Keeping these stages separate preserves
//! that distinction and allows one interpreted header to serve several reads.
//! It also prevents storage adapters from learning the SDK's field layout.
//!
//! # Why store derived regions?
//!
//! [`HeaderSnapshotReader`] stores [`HeaderRegions`] rather than rereading the
//! header before each operation. Every extraction is therefore governed by one
//! explicit header observation. This does not make live reads atomic, but it
//! prevents accidental mixing of offsets and lengths from different header
//! observations; the live reader can then apply its consistency protocol around
//! that stable interpretation.

use crate::{
    IRacingSDKError, Result,
    types::{
        FrameBuffer, Header, SessionInfoBuffer, VariableBuffer, VariableHeader,
        VariableHeadersBuffer, WireType,
    },
};

use super::access_source::{ByteRegion, RandomAccessSource};

/// Source-independent regions and frame length advertised by one SDK header.
///
/// The value owns no source and performs no I/O. Its regions have overflow-safe
/// endpoints, but are not known to fit a particular source until passed to
/// [`RandomAccessSource::validate_region`] or [`RandomAccessSource::snapshot`].
///
/// A zero session-info length or zero variable count is represented as `None`.
/// A zero frame length remains `0`, allowing source-specific validation to
/// decide whether a metadata-only IBT is legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderRegions {
    /// Optional byte range containing the session YAML snapshot.
    session_info: Option<ByteRegion>,
    /// Optional byte range containing the fixed-size variable-header array.
    variable_headers: Option<ByteRegion>,
    /// Byte length of each telemetry frame advertised by the header.
    frame_length: usize,
}

impl HeaderRegions {
    /// Interprets address-related fields from one header snapshot.
    ///
    /// Variable-header length is calculated as `variable_count *
    /// VariableHeader::WIRE_SIZE`. This method validates representation and
    /// arithmetic only; it does not validate regions against a source or apply
    /// live/IBT-specific header rules.
    ///
    /// # Errors
    ///
    /// Returns a parse or memory-access error when an advertised count, offset,
    /// or length is negative, cannot be represented as `usize`, or overflows
    /// during range calculation.
    pub fn from_header(header: &Header) -> Result<Self> {
        let session_info = optional_region(
            "session info",
            header.session_info_offset,
            header.session_info_len,
        )?;

        let variable_count = usize::try_from(header.variable_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "SDK header regions",
                format!(
                    "Variable count cannot be negative: {}",
                    header.variable_count
                ),
            )
        })?;
        let variable_length = variable_count
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "SDK header regions",
                    "Variable-header region length overflowed",
                )
            })?;
        let variable_headers = optional_region(
            "variable headers",
            header.variable_header_offset,
            i32::try_from(variable_length).map_err(|_| {
                IRacingSDKError::parse_error(
                    "SDK header regions",
                    "Variable-header region length exceeds the SDK offset range",
                )
            })?,
        )?;

        let frame_length = usize::try_from(header.buffer_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "SDK header regions",
                format!("Frame length cannot be negative: {}", header.buffer_length),
            )
        })?;

        Ok(Self {
            session_info,
            variable_headers,
            frame_length,
        })
    }

    /// Returns the optional session-information byte region.
    pub fn session_info(self) -> Option<ByteRegion> {
        self.session_info
    }

    /// Returns the optional variable-header array byte region.
    pub fn variable_headers(self) -> Option<ByteRegion> {
        self.variable_headers
    }

    /// Returns the advertised byte length of a telemetry frame.
    pub fn frame_length(self) -> usize {
        self.frame_length
    }

    /// Creates a frame-sized region at a caller-selected absolute offset.
    ///
    /// IBT layout code can use this with a calculated record offset. The live
    /// path normally uses [`Self::variable_buffer`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Memory`] if adding the frame length to the
    /// supplied offset overflows. Source containment is checked later.
    pub fn frame_at(self, offset: usize) -> Result<ByteRegion> {
        ByteRegion::new(offset, self.frame_length)
    }

    /// Creates the frame region described by a live buffer descriptor.
    ///
    /// The region uses `buffer.buffer_offset` and this header's common frame
    /// length. It does not inspect the descriptor's tick fields.
    ///
    /// # Errors
    ///
    /// Returns a parse error for a negative buffer offset or a memory-access
    /// error if the resulting end offset overflows. Source containment and tick
    /// consistency are intentionally deferred to later layers.
    pub fn variable_buffer(self, buffer: &VariableBuffer) -> Result<ByteRegion> {
        let offset = usize::try_from(buffer.buffer_offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "SDK header regions",
                format!(
                    "Variable buffer offset cannot be negative: {}",
                    buffer.buffer_offset
                ),
            )
        })?;

        self.frame_at(offset)
    }
}

/// Extracts typed, owned snapshots using one interpreted SDK header.
///
/// The reader borrows a source and stores a copied [`HeaderRegions`] value, so
/// every method uses the same offsets and lengths even if the original header
/// memory later changes. Each returned buffer owns its bytes.
///
/// This type does not coordinate multiple reads. Against live memory, a caller
/// must still apply the SDK's tick/version consistency protocol around each
/// extraction that requires a coherent observation.
pub struct HeaderSnapshotReader<'source, S: ?Sized> {
    /// Addressable source from which advertised regions are copied.
    source: &'source S,
    /// Immutable interpretation of the header used for all reads.
    regions: HeaderRegions,
}

impl<'source, S: RandomAccessSource + ?Sized> HeaderSnapshotReader<'source, S> {
    /// Creates a header-directed reader over an existing source.
    ///
    /// Construction interprets header fields but performs no source reads. Each
    /// region is checked against the source when its corresponding method is
    /// called.
    ///
    /// # Errors
    ///
    /// Propagates invalid signed fields and arithmetic overflow from
    /// [`HeaderRegions::from_header`].
    pub fn new(source: &'source S, header: &Header) -> Result<Self> {
        Ok(Self {
            source,
            regions: HeaderRegions::from_header(header)?,
        })
    }

    /// Returns the immutable regions used by this reader.
    ///
    /// This supports source-specific layout code without requiring it to
    /// reinterpret signed header fields.
    pub fn regions(&self) -> HeaderRegions {
        self.regions
    }

    /// Copies the advertised session-information region into an owned buffer.
    ///
    /// Returns `Ok(None)` when the header advertises a zero-length session-info
    /// region. An advertised but empty-in-content region is still returned as
    /// `Some`; textual decoding decides whether its contents are meaningful.
    ///
    /// # Errors
    ///
    /// Propagates an out-of-bounds or source-copy error. No partial buffer is
    /// returned.
    pub fn session_info_buffer(&self) -> Result<Option<SessionInfoBuffer>> {
        self.regions
            .session_info()
            .map(|region| {
                self.source
                    .snapshot(region)
                    .map(SessionInfoBuffer::from_snapshot)
            })
            .transpose()
    }

    /// Copies the advertised variable-header array into an owned buffer.
    ///
    /// Returns `Ok(None)` when `variable_count` is zero. Individual wire headers
    /// are not semantically validated here; that occurs when the buffer is
    /// converted into schema metadata.
    ///
    /// # Errors
    ///
    /// Propagates an out-of-bounds or source-copy error. No partial array is
    /// returned.
    pub fn variable_headers_buffer(&self) -> Result<Option<VariableHeadersBuffer>> {
        self.regions
            .variable_headers()
            .map(|region| {
                self.source
                    .snapshot(region)
                    .map(VariableHeadersBuffer::from_snapshot)
            })
            .transpose()
    }

    /// Copies one frame-sized snapshot beginning at an absolute offset.
    ///
    /// This is the random-access primitive used by an IBT layout/cursor layer.
    /// The method does not assign a tick, session version, or frame index.
    ///
    /// # Errors
    ///
    /// Returns an error when the calculated frame range overflows, lies outside
    /// the source, or cannot be copied completely.
    pub fn frame_at(&self, offset: usize) -> Result<FrameBuffer> {
        let bytes = self.source.snapshot(self.regions.frame_at(offset)?)?;
        Ok(FrameBuffer::from_snapshot(bytes))
    }

    /// Copies the frame described by a live variable-buffer descriptor.
    ///
    /// Only the descriptor's byte offset is consumed here. Tick selection,
    /// before/after validation, retry behavior, and newest-buffer policy belong
    /// to the live reader.
    ///
    /// # Errors
    ///
    /// Returns an error for a negative or overflowing offset, an out-of-bounds
    /// frame, or a source-copy failure.
    pub fn variable_buffer(&self, buffer: &VariableBuffer) -> Result<FrameBuffer> {
        let bytes = self
            .source
            .snapshot(self.regions.variable_buffer(buffer)?)?;
        Ok(FrameBuffer::from_snapshot(bytes))
    }
}

/// Converts one signed SDK offset/length pair into an optional checked region.
///
/// Zero length means the region is absent, but the offset is still required to
/// be non-negative so malformed headers are not silently accepted.
fn optional_region(context: &str, offset: i32, length: i32) -> Result<Option<ByteRegion>> {
    if length < 0 {
        return Err(IRacingSDKError::parse_error(
            "SDK header regions",
            format!("{context} length cannot be negative: {length}"),
        ));
    }

    let offset = usize::try_from(offset).map_err(|_| {
        IRacingSDKError::parse_error(
            "SDK header regions",
            format!("{context} offset cannot be negative: {offset}"),
        )
    })?;

    if length == 0 {
        return Ok(None);
    }

    let length = usize::try_from(length).map_err(|_| {
        IRacingSDKError::parse_error(
            "SDK header regions",
            format!("{context} length cannot be represented: {length}"),
        )
    })?;

    ByteRegion::new(offset, length).map(Some)
}

#[cfg(test)]
mod tests {
    use super::super::access_source::OwnedBytes;
    use super::*;
    use crate::types::{IRSDK_VERSION, VariableBuffer, irsdk::StatusField};

    fn header() -> Header {
        Header::new(
            IRSDK_VERSION,
            StatusField::CONNECTED,
            60,
            7,
            4,
            8,
            1,
            12,
            1,
            3,
            10,
            0,
            [
                VariableBuffer::new(10, 156, 10),
                VariableBuffer::new(0, 0, 0),
                VariableBuffer::new(0, 0, 0),
                VariableBuffer::new(0, 0, 0),
            ],
        )
    }

    #[test]
    fn header_regions_calculates_advertised_ranges() -> Result<()> {
        let regions = HeaderRegions::from_header(&header())?;

        assert_eq!(regions.session_info(), Some(ByteRegion::new(8, 4)?));
        assert_eq!(
            regions.variable_headers(),
            Some(ByteRegion::new(12, VariableHeader::WIRE_SIZE)?)
        );
        assert_eq!(regions.frame_length(), 3);
        assert_eq!(regions.frame_at(156)?, ByteRegion::new(156, 3)?);
        Ok(())
    }

    #[test]
    fn snapshot_reader_returns_typed_owned_snapshots() -> Result<()> {
        let mut bytes = vec![0; 159];
        bytes[8..12].copy_from_slice(b"yaml");
        bytes[156..159].copy_from_slice(&[1, 2, 3]);
        let source = OwnedBytes::from(bytes);
        let header = header();
        let reader = HeaderSnapshotReader::new(&source, &header)?;

        let session: String = reader
            .session_info_buffer()?
            .expect("session snapshot")
            .into();
        let frame: Vec<u8> = reader.frame_at(156)?.into();

        assert_eq!(session, "yaml");
        assert_eq!(frame, [1, 2, 3]);
        assert!(reader.variable_headers_buffer()?.is_some());
        Ok(())
    }

    #[test]
    fn snapshot_reader_preserves_out_of_bounds_failures() -> Result<()> {
        let source = OwnedBytes::from(vec![0; 16]);
        let header = header();
        let reader = HeaderSnapshotReader::new(&source, &header)?;

        assert!(reader.variable_headers_buffer().is_err());
        assert!(reader.variable_buffer(&header.buffers[0]).is_err());
        Ok(())
    }

    #[test]
    fn header_regions_rejects_negative_fields() {
        let mut invalid_header = header();
        invalid_header.session_info_len = -1;
        assert!(HeaderRegions::from_header(&invalid_header).is_err());

        let mut invalid_header = header();
        invalid_header.variable_count = -1;
        assert!(HeaderRegions::from_header(&invalid_header).is_err());

        let mut invalid_header = header();
        invalid_header.buffer_length = -1;
        assert!(HeaderRegions::from_header(&invalid_header).is_err());
    }
}
