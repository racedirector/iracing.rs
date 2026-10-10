//! Indexed IBT file reader
//!
//! Provides cross-platform indexed frame reads and owned metadata snapshots.
//! Sequential replay belongs to `super::IbtReplay`; providers construct schemas.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use iracing_sdk::ibt::IbtReader;
//!
//! fn read_frames() -> iracing_sdk::Result<()> {
//!     // Open IBT file
//!     let reader = IbtReader::open("telemetry.ibt")?;
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

use crate::{
    IRacingSDKError, IbtLayout, Result, SessionInfoBytes, VariableHeaders,
    provider::{SessionInformationBytesProvider, VariableHeadersProvider},
    source::ibt::Source,
};
use memmap2::Mmap;
use std::{fs::File, ops::Range, path::Path};

use iracing_irsdk::{DiskSubHeader, Header, IbtHeader};

/// Low-level IBT reader for indexed frames and fresh metadata snapshots.
///
/// This reader has no logical replay cursor and does not construct a schema.
///
/// Geometry uses `usize`: sources larger than `usize::MAX` bytes are rejected
/// (including files of 4 GiB or more on 32-bit targets).
pub struct IbtReader {
    source: Source,

    header: IbtHeader,
    layout: IbtLayout,
}

/// One owned recorded frame with its zero-based file coordinate.
///
/// The index identifies a physical record in this recording, not a live SDK
/// tick or the compatibility synthetic tick in `FramePacket`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedFrame {
    index: usize,
    bytes: Vec<u8>,
}

impl RecordedFrame {
    /// Returns the zero-based record index within the recording.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the raw telemetry bytes, without a schema or session cache.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes this frame and returns its owned telemetry bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Demand-driven traversal of a validated half-open recorded frame range.
///
/// Each call to `next` reads at most one frame through `IbtLayout`. The reader
/// remains cursor-free; multiple ranges and direct reads are independent.
/// A read error is yielded for its index and traversal proceeds to the next
/// index. The iterator ends permanently at the exclusive bound.
pub struct IbtFrames<'a> {
    reader: &'a IbtReader,
    remaining: Range<usize>,
}

impl std::fmt::Debug for IbtFrames<'_> {
    /// Formats the remaining record range, propagating any formatter error.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbtFrames")
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

impl Iterator for IbtFrames<'_> {
    type Item = Result<RecordedFrame>;

    /// Reads the next record, yielding its index and owned telemetry bytes.
    ///
    /// Errors from [`IbtReader::frame`] are yielded unchanged and still advance
    /// the iterator to the next index. Returns `None` permanently once the
    /// exclusive range bound is reached.
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.remaining.next()?;
        Some(
            self.reader
                .frame(index)
                .map(|bytes| RecordedFrame { index, bytes }),
        )
    }

    /// Returns exact bounds on the remaining items, including any read errors.
    /// No frame bytes are read.
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}

impl ExactSizeIterator for IbtFrames<'_> {}
impl std::iter::FusedIterator for IbtFrames<'_> {}

impl IbtReader {
    /// Transfers existing mapped or owned storage into the completed file model.
    ///
    /// Temporary compatibility for IbtProvider::from_reader; removed by #305.
    /// Establishes immutable metadata without copying the complete recording.
    pub(crate) fn into_file(self) -> Result<super::IbtFile> {
        super::IbtFile::from_source(self.source)
    }

    /// Open and parse an immutable `.ibt` recording using a read-only memory map.
    ///
    /// The file must not be modified or truncated by any process while this reader
    /// is alive. Only open completed recordings whose storage you control. Use
    /// [`Self::from_bytes`] with an owned copy if this cannot be guaranteed.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;

        // SAFETY: This maps a completed recording read-only. The documented
        // open contract requires the backing file to remain unchanged for the
        // reader's lifetime. Mmap owns the mapping independently of `file` and
        // unmaps it on drop; no mapped references escape this reader.
        let mapped = unsafe { Mmap::map(&file) }.map_err(|error| IRacingSDKError::File {
            path,
            source: std::io::Error::new(error.kind(), format!("Failed to map IBT source: {error}")),
        })?;
        Self::from_source(Source::Mapped(mapped))
    }

    /// Parse owned in-memory `.ibt` data.
    pub fn from_bytes<B: Into<Vec<u8>>>(data: B) -> Result<Self> {
        Self::from_source(Source::Owned(data.into()))
    }

    fn from_source(source: Source) -> Result<Self> {
        let source_len = source.len();
        let preamble_len = size_of::<Header>() + size_of::<DiskSubHeader>();
        let Some(bytes) = source.get(0..preamble_len) else {
            return Err(IRacingSDKError::parse_error(
                "IbtReader::from_source",
                "Source is shorter than the IBT preamble",
            ));
        };

        let ibt_header = IbtHeader::try_from_bytes(bytes).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtReader::from_source",
                "Could not parse IBT header from source",
            )
        })?;

        let layout = IbtLayout::try_from_headers(ibt_header.header(), source_len)?;

        let disk_header = ibt_header.disk_header();
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
            header: ibt_header,
            layout,
        })
    }

    /// Returns the canonical physical layout of this source.
    ///
    /// The layout is constructed once when the source is opened. It exposes the
    /// fixed preamble, optional metadata, and indexed frame regions without
    /// additional source reads or changes to the source cursor.
    pub fn layout(&self) -> &IbtLayout {
        &self.layout
    }

    /// Reads owned session bytes. Import the provider trait to migrate.
    #[deprecated(
        note = "use iracing_sdk::provider::SessionInformationBytesProvider::session_info_snapshot"
    )]
    pub fn session_info_snapshot(&mut self) -> Result<Option<SessionInfoBytes>> {
        SessionInformationBytesProvider::session_info_snapshot(self)
    }

    /// Reads and decodes exactly the advertised variable-header records on each call.
    ///
    /// Returns `None` if absent. Semantic schema validation is left to the caller.
    ///
    /// # Errors
    /// Returns an error if reading or decoding the complete region fails.
    #[deprecated(
        note = "use iracing_sdk::provider::VariableHeadersProvider::variable_headers; absent metadata returns an empty snapshot"
    )]
    pub fn variable_headers_snapshot(&mut self) -> Result<Option<VariableHeaders>> {
        if self.layout.metadata().variable_headers().is_none() {
            return Ok(None);
        }

        self.variable_headers().map(Some)
    }

    /// Reads exactly one indexed frame, regardless of prior source reads.
    ///
    /// `index` is a zero-based physical record coordinate within this recording,
    /// not a byte offset or a live SDK tick. It must be less than
    /// [`Self::frame_count`]. Addressing delegates to [`IbtLayout::frame`] and
    /// uses O(1) addressing and never changes replay state.
    ///
    /// # Errors
    /// Returns an error if `index` is out of range or the complete frame cannot be read.
    pub fn frame(&self, index: usize) -> Result<Vec<u8>> {
        let frame = self.layout.frame(index)?;
        let frame_range = frame.as_region().as_range();

        let Some(bytes) = self.source.get(frame_range) else {
            return Err(IRacingSDKError::parse_error(
                "IbtReader::frame",
                format!("Could not get frame {} bytes from source", index),
            ));
        };

        Ok(bytes.into())
    }

    /// Traverses a half-open range of zero-based recorded frame indices.
    ///
    /// Coordinates are `usize`, matching [`Self::frame_count`] and the physical
    /// layout; they are not byte offsets or live SDK ticks. The end is clamped to
    /// `frame_count`; the start must not exceed the end or `frame_count`.
    /// Empty ranges, including the range at EOF,
    /// are allowed. Validation is O(1) and reads no source bytes. Each iterator
    /// item allocates only its requested frame, and no replay state is changed.
    ///
    /// ```no_run
    /// # use iracing_sdk::ibt::IbtReader;
    /// # fn inspect() -> iracing_sdk::Result<()> {
    /// let reader = IbtReader::open("telemetry.ibt")?;
    /// for frame in reader.frames(0..reader.frame_count())? {
    ///     let frame = frame?;
    ///     println!("record {}: {} bytes", frame.index(), frame.bytes().len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns a parse error before source access for reversed ranges or starts
    /// beyond the recording. Frame read failures are returned by the iterator.
    pub fn frames(&self, range: Range<usize>) -> Result<IbtFrames<'_>> {
        if range.start > range.end || range.start > self.frame_count() {
            return Err(IRacingSDKError::parse_error(
                "IbtReader::frames",
                format!(
                    "Invalid frame range {range:?} for {} frames",
                    self.frame_count()
                ),
            ));
        }
        Ok(IbtFrames {
            reader: self,
            remaining: range.start..range.end.min(self.frame_count()),
        })
    }

    /// Traverses all recorded frames in increasing index order, starting at zero.
    ///
    /// Equivalent to `self.frames(0..self.frame_count())`. Construction always
    /// succeeds without reading frame bytes; an empty recording yields no items.
    /// Frame read errors are yielded by the iterator, which can continue with
    /// the next record.
    pub fn all_frames(&self) -> Result<IbtFrames<'_>> {
        self.frames(0..self.frame_count())
    }

    /// Get disk metadata from the disk sub-header
    pub fn disk_header(&self) -> &DiskSubHeader {
        self.header.disk_header()
    }

    /// Get the IBT header information
    pub fn header(&self) -> &Header {
        self.header.header()
    }

    /// The size of an individual frame.
    pub fn frame_size(&self) -> usize {
        self.layout.frame_size()
    }

    /// The total number of frames in the recording.
    pub fn frame_count(&self) -> usize {
        self.layout.frame_count()
    }
}

impl SessionInformationBytesProvider for IbtReader {
    fn session_info_snapshot(&self) -> Result<Option<SessionInfoBytes>> {
        let Some(region) = self.layout.metadata().session_info() else {
            return Ok(None);
        };

        let range = region.as_region().as_range();

        let Some(bytes) = self.source.get(range) else {
            return Err(IRacingSDKError::parse_error(
                "IbtReader::session_info_snapshot",
                "Could not get session info bytes from source",
            ));
        };

        let snapshot = SessionInfoBytes::from_checked_region(bytes);

        Ok(Some(snapshot))
    }
}

impl VariableHeadersProvider for IbtReader {
    fn variable_headers(&self) -> Result<VariableHeaders> {
        let Some(region) = self.layout.metadata().variable_headers() else {
            return Ok(VariableHeaders::default());
        };

        let range = region.as_region().as_range();

        let Some(bytes) = self.source.get(range) else {
            return Err(IRacingSDKError::parse_error(
                "IbtReader::variable_headers_snapshot",
                "Could not get variable headers bytes from source",
            ));
        };

        VariableHeaders::try_from_bytes(bytes, region.count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::{Context, Result};

    use std::fs::OpenOptions;
    use std::path::PathBuf;

    impl IbtReader {
        pub(crate) fn owned_bytes_mut(&mut self) -> &mut Vec<u8> {
            match &mut self.source {
                Source::Owned(bytes) => bytes,
                Source::Mapped(_) => panic!("fault injection requires an owned source"),
            }
        }
    }

    #[test]
    fn indexed_reads_are_independent_of_metadata_and_invalid_indices() -> Result<()> {
        for reader in [
            IbtReader::open(fixture_path()?)?,
            IbtReader::from_bytes(fixture_bytes()?)?,
        ] {
            let first = reader.frame(0)?;
            let last_index = reader.layout().frame_count() - 1;
            let last = reader.frame(last_index)?;
            SessionInformationBytesProvider::session_info_snapshot(&reader)?;
            reader.variable_headers()?;
            assert!(reader.frame(last_index + 1).is_err());
            assert!(reader.frame(usize::MAX).is_err());
            assert_eq!(reader.frame(0)?, first);
            assert_eq!(reader.frame(last_index)?, last);
        }
        Ok(())
    }

    #[test]
    fn ranges_are_bounded_independent_and_preserve_record_identity() -> Result<()> {
        for reader in [
            IbtReader::open(fixture_path()?)?,
            IbtReader::from_bytes(fixture_bytes()?)?,
        ] {
            let count = reader.frame_count();
            assert!(count >= 3);
            for range in [0..0, count..count, 0..1, 1..3, count - 1..count] {
                let mut frames = reader.frames(range.clone())?;
                assert_eq!(frames.len(), range.len());
                for index in range {
                    let frame = frames.next().context("expected frame")??;
                    assert_eq!(frame.index(), index);
                    assert_eq!(frame.bytes(), reader.frame(index)?);
                    assert_eq!(frame.clone().into_bytes(), reader.frame(index)?);
                }
                assert_eq!(frames.len(), 0);
                assert!(frames.next().is_none());
                assert!(frames.next().is_none());
            }
            for end in [count + 1, usize::MAX] {
                let frames = reader.frames(count - 1..end)?;
                assert_eq!(frames.len(), 1);
                let frames = frames.collect::<crate::Result<Vec<_>>>()?;
                assert_eq!(frames[0].index(), count - 1);
                assert_eq!(frames[0].bytes(), reader.frame(count - 1)?);
                assert!(reader.frames(count..end)?.next().is_none());
            }
            let mut first = reader.frames(0..2)?;
            let mut second = reader.frames(count - 1..count)?;
            assert_eq!(first.next().unwrap()?.index(), 0);
            assert_eq!(second.next().unwrap()?.index(), count - 1);
            assert_eq!(first.next().unwrap()?.index(), 1);
            for range in [Range { start: 2, end: 1 }, count + 1..count + 1] {
                assert!(reader.frames(range).is_err());
            }
        }
        Ok(())
    }

    #[test]
    fn ranges_validate_before_source_access_and_read_only_on_demand() -> Result<()> {
        let mut reader = IbtReader::from_bytes(fixture_bytes()?)?;
        let count = reader.frame_count();
        let first_end = reader.layout().frame(0)?.end();
        reader.owned_bytes_mut().truncate(first_end);
        assert_eq!(reader.frames(0..count + 1)?.len(), count);
        assert!(reader.frames(count + 1..usize::MAX).is_err());
        assert!(reader.frames(count..count)?.next().is_none());
        let mut frames = reader.frames(0..count)?;
        assert!(frames.next().unwrap().is_ok());
        assert!(frames.next().unwrap().is_err());
        assert_eq!(reader.frame(0)?.len(), reader.frame_size());
        Ok(())
    }

    #[test]
    fn empty_recording_allows_only_empty_range() -> Result<()> {
        let original = fixture_bytes()?;
        let end = IbtReader::from_bytes(original.clone())?
            .layout()
            .frame_data_start();
        let reader = IbtReader::from_bytes(original[..end].to_vec())?;
        assert_eq!(reader.frame_count(), 0);
        assert!(reader.frames(0..0)?.next().is_none());
        assert!(reader.frames(0..1)?.next().is_none());
        assert!(reader.frames(0..usize::MAX)?.next().is_none());
        assert!(reader.frames(1..usize::MAX).is_err());
        assert!(reader.frame(0).is_err());
        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn legacy_metadata_accessors_preserve_absence() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        write_i32(&mut bytes, 16, 0);
        write_i32(&mut bytes, 24, 0);
        bytes.truncate(size_of::<Header>() + size_of::<DiskSubHeader>());
        let mut reader = IbtReader::from_bytes(bytes)?;
        assert!(reader.session_info_snapshot()?.is_none());
        assert!(reader.variable_headers_snapshot()?.is_none());
        assert!(reader.variable_headers()?.is_empty());
        Ok(())
    }

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
    fn file_and_owned_headers_and_layout_match() -> Result<()> {
        use zerocopy::IntoBytes;

        let path = fixture_path()?;
        let file = IbtReader::open(&path)?;
        let owned = IbtReader::from_bytes(std::fs::read(path)?)?;
        assert_eq!(file.header().as_bytes(), owned.header().as_bytes());
        assert_eq!(
            file.disk_header().as_bytes(),
            owned.disk_header().as_bytes()
        );
        assert_eq!(
            file.layout().metadata().session_info(),
            owned.layout().metadata().session_info()
        );
        assert_eq!(
            file.layout().metadata().variable_headers(),
            owned.layout().metadata().variable_headers()
        );
        assert_eq!(file.layout().frames(), owned.layout().frames());
        Ok(())
    }

    #[test]
    fn malformed_file_and_owned_sources_fail_equivalently() -> Result<()> {
        let original = fixture_bytes()?;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("malformed.ibt");
        let preamble_size = size_of::<Header>() + size_of::<DiskSubHeader>();
        let mut cases = vec![
            original[..1].to_vec(),
            original[..size_of::<Header>() - 1].to_vec(),
            original[..preamble_size - 1].to_vec(),
            original[..original.len() - 1].to_vec(),
        ];
        for (offset, value) in [(28, i32::MAX), (20, i32::MAX), (36, 0)] {
            let mut bytes = original.clone();
            write_i32(&mut bytes, offset, value);
            cases.push(bytes);
        }
        for bytes in cases {
            std::fs::write(&path, &bytes)?;
            let file_error = IbtReader::open(&path).err().context("file must fail")?;
            let owned_error = IbtReader::from_bytes(bytes)
                .err()
                .context("owned source must fail")?;
            assert_eq!(file_error.to_string(), owned_error.to_string());
        }
        // Empty files may fail at mapping rather than parsing on some platforms.
        std::fs::write(&path, [])?;
        assert!(IbtReader::open(&path).is_err());
        assert!(IbtReader::from_bytes(Vec::new()).is_err());
        Ok(())
    }

    #[test]
    fn dropping_reader_releases_file_resources() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("drop.ibt");
        std::fs::copy(fixture_path()?, &path)?;
        let reader = IbtReader::open(&path)?;
        let frame = reader.frame(0)?;
        drop(reader);
        // Windows disallows truncating a file with a live file mapping.
        OpenOptions::new().write(true).open(&path)?.set_len(0)?;
        std::fs::remove_file(&path)?;
        assert!(!frame.is_empty());
        Ok(())
    }

    #[test]
    fn truncated_main_header_is_rejected() -> Result<()> {
        let bytes = fixture_bytes()?;
        assert!(IbtReader::from_bytes(bytes[..size_of::<Header>() - 1].to_vec()).is_err());
        Ok(())
    }

    #[test]
    fn every_truncated_preamble_length_is_rejected() {
        let preamble_size = size_of::<Header>() + size_of::<DiskSubHeader>();
        for len in 0..preamble_size {
            let error = IbtReader::from_bytes(vec![0; len])
                .err()
                .expect("truncated preamble must fail");
            assert!(
                matches!(error, IRacingSDKError::Parse { .. }),
                "unexpected error for source length {len}: {error}"
            );
            assert!(
                error
                    .to_string()
                    .contains("Source is shorter than the IBT preamble")
            );
        }
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
            let reader = IbtReader::from_bytes(bytes)?;
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
    fn from_bytes_builds_an_owned_memory_reader() -> Result<()> {
        let test_file = fixture_path()?;
        let data = std::fs::read(test_file)?;

        let reader = IbtReader::from_bytes(data)?;

        assert!(matches!(reader.source, Source::Owned(_)));
        assert!(reader.layout().frame_count() > 0);
        assert!(!reader.variable_headers()?.is_empty());
        Ok(())
    }

    #[test]
    fn open_keeps_frames_mapped() -> Result<()> {
        let reader = IbtReader::open(fixture_path()?)?;
        assert!(matches!(reader.source, Source::Mapped(_)));
        Ok(())
    }
}
