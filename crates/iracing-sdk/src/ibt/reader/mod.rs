//! Indexed IBT file reader
//!
//! Provides cross-platform indexed frame reads and owned metadata snapshots.
//! Sequential replay and schema construction belong to `IbtProvider`.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use iracing_sdk::ibt::IbtReader;
//!
//! fn read_frames() -> iracing_sdk::Result<()> {
//!     // Open IBT file
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     println!("File contains {} frames", reader.layout().frame_count());
//!
//!     for index in 0..reader.layout().frame_count() {
//!         let frame = reader.frame(index)?;
//!         println!("Frame {index}: {} bytes", frame.len());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Performance Notes
//!
//! - Files remain file-backed; construction retains only the source and parsed metadata
//! - Frame reading is allocation-minimal except for the returned frame bytes
//! - Indexed frame geometry is O(1)

mod source;

use crate::{
    IRacingSDKError, IbtLayout, Result, SessionInfoBuffer, VariableHeadersBuffer,
    irsdk::{DiskSubHeader, Header},
};
use source::IbtSource;
use std::{
    fs::File,
    io::{Cursor, Seek, SeekFrom},
    path::Path,
};

/// Low-level IBT reader for indexed frames and fresh metadata snapshots.
///
/// This reader has no logical replay cursor and does not construct a schema.
///
/// Geometry uses `usize`: sources larger than `usize::MAX` bytes are rejected
/// (including files of 4 GiB or more on 32-bit targets).
pub struct IbtReader {
    source: IbtSource,

    header: Header,
    disk_header: DiskSubHeader,
    layout: IbtLayout,
}

impl IbtReader {
    /// Open and parse an `.ibt` file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;

        Self::from_source(IbtSource::File(file))
    }

    /// Parse owned in-memory `.ibt` data.
    pub fn from_bytes<B: Into<Vec<u8>>>(data: B) -> Result<Self> {
        Self::from_source(IbtSource::Memory(Cursor::new(data.into())))
    }

    fn from_source(mut source: IbtSource) -> Result<Self> {
        let source_len = source.len()?;
        source.seek(SeekFrom::Start(0)).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT source seek",
                format!("Failed to seek to source start: {error}"),
            )
        })?;

        // Parse IBT header
        let header = Header::try_from_reader(&mut source)?;
        // Parse disk sub-header (note: may be corrupted, but we'll try)
        let disk_header = DiskSubHeader::try_from_reader(&mut source)?;

        let layout = IbtLayout::try_from_headers(&header, source::layout_source_len(source_len)?)?;

        // Record counts are advisory; only the layout determines EOF.
        if disk_header.record_count > 0
            && layout.frame_count() > 0
            && usize::try_from(disk_header.record_count).ok() != Some(layout.frame_count())
        {
            tracing::warn!(
                "Frame count mismatch: disk header reports {} records, calculated {} frames from file size",
                disk_header.record_count,
                layout.frame_count()
            );
        }

        Ok(Self {
            source,
            header,
            disk_header,
            layout,
        })
    }

    /// Returns the validated metadata and frame geometry.
    pub fn layout(&self) -> &IbtLayout {
        &self.layout
    }

    /// Reads an owned snapshot of exactly the advertised session region on each call.
    ///
    /// Returns `None` if absent. Each snapshot owns its bytes independently of the source.
    ///
    /// # Errors
    /// Returns an error if the source cannot supply the complete region.
    pub fn session_info_snapshot(&mut self) -> Result<Option<SessionInfoBuffer>> {
        self.layout
            .metadata()
            .session_info()
            .map(|region| {
                self.source
                    .read_region(region.as_region())
                    .map(SessionInfoBuffer::from_owned_checked_region)
            })
            .transpose()
    }

    /// Reads and decodes exactly the advertised variable-header records on each call.
    ///
    /// Returns `None` if absent. Semantic schema validation is left to the caller.
    ///
    /// # Errors
    /// Returns an error if reading or decoding the complete region fails.
    pub fn variable_headers_snapshot(&mut self) -> Result<Option<VariableHeadersBuffer>> {
        self.layout
            .metadata()
            .variable_headers()
            .map(|region| {
                let bytes = self.source.read_region(region.as_region())?;
                VariableHeadersBuffer::try_from_region_bytes(&bytes, region.count())
            })
            .transpose()
    }

    /// Reads exactly one indexed frame, regardless of prior source reads.
    ///
    /// # Errors
    /// Returns an error if `index` is out of range or the complete frame cannot be read.
    pub fn frame(&mut self, index: usize) -> Result<Vec<u8>> {
        self.source
            .read_region(self.layout.frame(index)?.as_region())
    }

    /// Get disk metadata from the disk sub-header
    pub fn disk_header(&self) -> &DiskSubHeader {
        &self.disk_header
    }

    /// Get the IBT header information
    pub fn header(&self) -> &Header {
        &self.header
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::{Context, Result};

    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
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

    #[test]
    fn advisory_record_count_does_not_define_frame_bounds() -> Result<()> {
        let original = fixture_bytes()?;
        let expected = IbtReader::from_bytes(original.clone())?
            .layout()
            .frame_count();
        for record_count in [-1, 0, 1, i32::MAX] {
            let mut bytes = original.clone();
            write_i32(&mut bytes, size_of::<Header>() + 28, record_count);
            let mut reader = IbtReader::from_bytes(bytes)?;
            assert_eq!(reader.layout().frame_count(), expected);
            assert_eq!(
                reader.frame(expected - 1)?.len(),
                reader.layout().frame_size()
            );
            assert!(reader.frame(expected).is_err());
        }
        Ok(())
    }

    #[test]
    fn invalid_index_preserves_source_position() -> Result<()> {
        let mut reader = IbtReader::from_bytes(fixture_bytes()?)?;
        let position = reader.source.stream_position()?;
        assert!(reader.frame(reader.layout().frame_count()).is_err());
        assert_eq!(reader.source.stream_position()?, position);
        Ok(())
    }

    #[test]
    fn truncation_after_construction_is_a_read_error() -> Result<()> {
        let temporary_directory = tempfile::tempdir()?;
        let temporary_file = temporary_directory.path().join("truncated-after-open.ibt");
        std::fs::copy(fixture_path()?, &temporary_file)?;
        let mut reader = IbtReader::open(&temporary_file)?;
        let truncated_len = u64::try_from(reader.layout.frame_data_start())?
            .checked_add(u64::try_from(reader.layout.frame_size())? - 1)
            .context("truncated fixture length should fit")?;
        OpenOptions::new()
            .write(true)
            .open(&temporary_file)?
            .set_len(truncated_len)?;

        assert!(reader.frame(0).is_err());
        assert_eq!(
            reader.source.stream_position()?,
            u64::try_from(reader.layout.frame_data_start())?
        );
        Ok(())
    }

    #[test]
    fn from_bytes_builds_an_owned_memory_reader() -> Result<()> {
        let test_file = fixture_path()?;
        let data = std::fs::read(test_file)?;

        let mut reader = IbtReader::from_bytes(data)?;

        assert!(reader.layout().frame_count() > 0);
        assert!(!reader.variable_headers_snapshot()?.unwrap().is_empty());
        Ok(())
    }

    #[test]
    fn open_keeps_frames_file_backed() -> Result<()> {
        let temporary_directory = tempfile::tempdir()?;
        let temporary_file = temporary_directory.path().join("file-backed.ibt");
        std::fs::copy(fixture_path()?, &temporary_file)?;
        let mut reader = IbtReader::open(&temporary_file)?;

        let replacement = 0xA5;
        let mut file = OpenOptions::new().write(true).open(&temporary_file)?;
        file.seek(SeekFrom::Start(u64::try_from(
            reader.layout.frame_data_start(),
        )?))?;
        file.write_all(&[replacement])?;
        file.flush()?;

        let frame = reader.frame(0)?;
        assert_eq!(frame[0], replacement);
        Ok(())
    }
}
