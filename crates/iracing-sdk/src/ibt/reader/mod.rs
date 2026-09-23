//! Validated, indexed access to iRacing `.ibt` telemetry files.
//!
//! [`IbtReader`] parses the fixed file headers, validates the advertised byte
//! regions through [`IbtLayout`], and performs exact reads for requested
//! metadata snapshots and telemetry frames. It deliberately does not maintain
//! a logical replay position or build a [`crate::VariableSchema`]; those are
//! higher-level provider responsibilities.
//!
//! Readers opened from a path remain file-backed. Readers constructed with
//! [`IbtReader::from_bytes`] own the supplied in-memory buffer. Every snapshot
//! and frame returned by this module is independently owned and does not borrow
//! from the reader.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use iracing_sdk::ibt::IbtReader;
//!
//! fn inspect_recording() -> iracing_sdk::Result<()> {
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     println!("File contains {} frames", reader.layout().frame_count());
//!
//!     if reader.layout().frame_count() > 0 {
//!         let first_frame = reader.frame(0)?;
//!         println!("First frame contains {} bytes", first_frame.len());
//!     }
//!
//!     if let Some(headers) = reader.variable_headers_snapshot()? {
//!         println!("File advertises {} telemetry variables", headers.len());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Performance Notes
//!
//! - [`IbtReader::open`] retains a file handle, the fixed headers, and the
//!   validated layout; it does not load the recording into memory.
//! - [`IbtReader::from_bytes`] retains the caller-supplied byte vector.
//! - Each metadata snapshot and frame read allocates only its returned owned
//!   buffer, apart from temporary decoding storage used by variable headers.
//! - Indexed reads seek directly to the validated region and perform one exact
//!   read. Metadata is read on demand and is not cached by the reader.

use crate::{
    ByteRegion, IRacingSDKError, IbtLayout, Result, SessionInfoBuffer, VariableHeadersBuffer,
    irsdk::{DiskSubHeader, Header},
};
use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
};

enum IbtSource {
    File(File),
    Memory(Cursor<Vec<u8>>),
}

impl Read for IbtSource {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(source) => source.read(buffer),
            Self::Memory(source) => source.read(buffer),
        }
    }
}

impl Seek for IbtSource {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::File(source) => source.seek(position),
            Self::Memory(source) => source.seek(position),
        }
    }
}

impl IbtSource {
    fn len(&self) -> Result<u64> {
        match self {
            Self::File(source) => Ok(source
                .metadata()
                .map_err(|error| {
                    IRacingSDKError::parse_error(
                        "IbtSource::len",
                        format!("Could not determine length of file: {error}"),
                    )
                })?
                .len()),
            Self::Memory(source) => Ok(source.get_ref().len() as u64),
        }
    }

    /// Seeks the underlying source to the first byte of a validated region.
    fn seek_to_region_start(&mut self, region: ByteRegion) -> Result<u64> {
        self.seek(SeekFrom::Start(region.offset() as u64))
            .map_err(|error| {
                IRacingSDKError::parse_error(
                    "Frame data seek",
                    format!("Failed to seek to {}: {error}", region.offset()),
                )
            })
    }

    /// Copies a complete validated region into a new owned byte vector.
    fn read_region(&mut self, region: ByteRegion) -> Result<Vec<u8>> {
        let mut bytes = vec![0; region.len()];
        self.read_region_into(region, &mut bytes)?;

        Ok(bytes)
    }

    /// Seeks to `region` and fills `buffer` with an exact read.
    ///
    /// The caller must provide a buffer whose length matches the region.
    fn read_region_into(&mut self, region: ByteRegion, buffer: &mut [u8]) -> Result<()> {
        self.seek_to_region_start(region)?;
        self.read_exact(buffer)
            .map_err(|e| IRacingSDKError::memory_access_error(region.offset(), e))
    }
}

/// Low-level reader for validated `.ibt` regions.
///
/// The reader owns its source and exposes random access by frame index. Calls
/// may change the source's physical seek position, but no logical playback
/// cursor is maintained. Sequential playback state belongs to
/// [`crate::providers::ibt::IbtProvider`].
pub struct IbtReader {
    source: IbtSource,
    header: Header,
    disk_header: DiskSubHeader,
    layout: IbtLayout,
}

impl IbtReader {
    /// Opens an `.ibt` file and validates its binary layout.
    ///
    /// Construction reads the fixed main and disk sub-headers. Metadata bodies
    /// and telemetry frames remain file-backed until explicitly requested.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be opened, its fixed headers
    /// cannot be read, or its advertised layout is invalid for the file length.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;

        Self::from_source(IbtSource::File(file))
    }

    /// Takes ownership of in-memory `.ibt` data and validates its binary layout.
    ///
    /// The supplied data becomes the reader's backing source without an
    /// additional full-buffer copy.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed headers cannot be read or their
    /// advertised layout is invalid for the supplied buffer length.
    pub fn from_bytes<B: Into<Vec<u8>>>(data: B) -> Result<Self> {
        Self::from_source(IbtSource::Memory(Cursor::new(data.into())))
    }

    fn from_source(mut source: IbtSource) -> Result<Self> {
        let source_len = source.len()?;
        source.seek(SeekFrom::Start(0)).map_err(|error| {
            IRacingSDKError::parse_error("IBTSource.seek", format!("Failed to seek to 0: {error}"))
        })?;

        let header = Header::try_from_reader(&mut source)?;
        let disk_header = DiskSubHeader::try_from_reader(&mut source)?;
        let layout = IbtLayout::try_from_headers(
            &header,
            usize::try_from(source_len).map_err(|_| {
                IRacingSDKError::parse_error(
                    "IbtReader::from_source",
                    "Could not convert detected source_len to usize",
                )
            })?,
        )?;

        Ok(IbtReader {
            source,
            header,
            disk_header,
            layout,
        })
    }

    /// Returns the parsed disk sub-header.
    pub fn disk_header(&self) -> &DiskSubHeader {
        &self.disk_header
    }

    /// Returns the parsed main SDK header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Returns the validated physical layout of metadata and frame regions.
    pub fn layout(&self) -> &IbtLayout {
        &self.layout
    }

    /// Reads the complete advertised session-information region.
    ///
    /// Returns `Ok(None)` when the header advertises no session-information
    /// region. A present region is copied on every call into an owned
    /// [`SessionInfoBuffer`]; the snapshot does not borrow from the reader and
    /// has not yet been decoded or parsed as YAML.
    ///
    /// # Errors
    ///
    /// Returns an error if the validated region cannot be read in full.
    pub fn session_info_snapshot(&mut self) -> Result<Option<SessionInfoBuffer>> {
        if let Some(session_info) = self.layout.metadata().session_info() {
            let bytes = self.source.read_region(session_info.as_region())?;
            let buffer = SessionInfoBuffer::from_owned_checked_region(bytes);
            return Ok(Some(buffer));
        }

        Ok(None)
    }

    /// Reads and decodes the complete advertised variable-header region.
    ///
    /// Returns `Ok(None)` when the header advertises no variable headers. A
    /// present region is copied and decoded into an owned
    /// [`VariableHeadersSnapshot`] on every call. Schema construction and
    /// semantic validation remain higher-level responsibilities.
    ///
    /// # Errors
    ///
    /// Returns an error if the region cannot be read in full or does not decode
    /// to exactly the advertised number of variable-header records.
    pub fn variable_headers_snapshot(&mut self) -> Result<Option<VariableHeadersBuffer>> {
        if let Some(variable_headers) = self.layout.metadata().variable_headers() {
            let bytes = self.source.read_region(variable_headers.as_region())?;
            let buffer =
                VariableHeadersBuffer::try_from_region_bytes(&bytes, variable_headers.count())?;

            return Ok(Some(buffer));
        }

        Ok(None)
    }

    /// Reads one complete telemetry frame by zero-based index.
    ///
    /// The returned vector owns exactly `layout().frame_size()` bytes and does
    /// not borrow from the reader.
    ///
    /// # Errors
    ///
    /// Returns an error when `index` is outside the validated frame range or
    /// when the frame region cannot be read in full.
    pub fn frame(&mut self, index: usize) -> Result<Vec<u8>> {
        let region = self.layout.frame(index)?;
        let bytes = self.source.read_region(region.as_region())?;

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::Result;

    use std::path::PathBuf;

    fn fixture_path() -> Result<PathBuf> {
        Ok(require_smallest_ibt_fixture()?)
    }

    fn fixture_bytes() -> Result<Vec<u8>> {
        Ok(std::fs::read(fixture_path()?)?)
    }

    fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
        bytes[offset..offset + std::mem::size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn truncated_main_header_is_rejected() -> Result<()> {
        let bytes = fixture_bytes()?;
        assert!(IbtReader::from_bytes(bytes[..size_of::<Header>() - 1].to_vec()).is_err());
        Ok(())
    }

    #[test]
    fn truncated_disk_sub_header_is_rejected() -> Result<()> {
        let bytes = fixture_bytes()?;
        let preamble_size = size_of::<Header>() + size_of::<DiskSubHeader>();
        assert!(IbtReader::from_bytes(bytes[..preamble_size - 1].to_vec()).is_err());
        Ok(())
    }

    #[test]
    fn variable_header_region_beyond_source_is_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let offset = i32::try_from(bytes.len() - 10)?;
        write_i32(&mut bytes, 28, offset);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }

    #[test]
    fn session_info_region_beyond_source_is_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let offset = i32::try_from(bytes.len() - 10)?;
        write_i32(&mut bytes, 16, 20);
        write_i32(&mut bytes, 20, offset);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }

    #[test]
    fn metadata_region_before_preamble_is_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        write_i32(&mut bytes, 28, 0);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }

    #[test]
    fn overlapping_metadata_regions_are_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let variable_offset = i32::from_le_bytes(bytes[28..32].try_into()?);
        write_i32(&mut bytes, 20, variable_offset);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }

    #[test]
    fn zero_frame_size_is_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        write_i32(&mut bytes, 36, 0);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }

    #[test]
    fn partial_trailing_frame_is_rejected() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        bytes.push(0);
        assert!(IbtReader::from_bytes(bytes).is_err());
        Ok(())
    }
}
