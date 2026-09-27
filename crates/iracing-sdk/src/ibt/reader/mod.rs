//! IBT file reader for telemetry replay
//!
//! Provides cross-platform IBT file reading for use with the ReplayProvider.
//! This allows IBT files to be replayed through the same architecture as live telemetry.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use iracing_sdk::ibt::IbtReader;
//!
//! fn read_frames() -> iracing_sdk::Result<()> {
//!     // Open IBT file
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     println!("File contains {} frames", reader.total_frames());
//!
//!     // Read frames sequentially
//!     while let Some((_frame_data, tick, session_version)) = reader.read_next_frame()? {
//!         println!("Frame at tick {} with session version {}",
//!             tick,
//!             session_version);
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
//! - Seeking operations are O(1)

mod source;

use super::format::extract_variable_schema;
use crate::{
    IRacingSDKError, IbtLayout, Result, SchemaProvider, SessionInfoBuffer, VariableHeadersBuffer,
    VariableSchema,
    irsdk::{DiskSubHeader, Header},
    types::IRacingSessionString,
};
use source::IbtSource;
use std::{
    fs::File,
    io::{Cursor, Seek, SeekFrom},
    path::Path,
};

/// IBT file reader for cross-platform replay.
///
/// Indexed reads and fresh metadata snapshots leave the logical replay cursor
/// unchanged. Sequential reads always seek to that logical position.
///
/// Geometry uses `usize`: sources larger than `usize::MAX` bytes are rejected
/// (including files of 4 GiB or more on 32-bit targets).
pub struct IbtReader {
    source: IbtSource,

    header: Header,
    disk_header: DiskSubHeader,
    layout: IbtLayout,

    // Temporary legacy replay/schema state, retained until provider cutover (#140).
    variable_schema: VariableSchema,

    session_info: Option<SessionInfoBuffer>,

    current_frame: usize,
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

        // Preserve the legacy empty-schema contract when no headers are advertised.
        let variable_schema = match layout.metadata().variable_headers() {
            Some(region) => extract_variable_schema(&mut source, region, layout.frame_size())?,
            None => VariableSchema::from_headers(
                &VariableHeadersBuffer::try_from_region_bytes(&[], 0)?,
                layout.frame_size(),
            )?,
        };
        let session_info = layout
            .metadata()
            .session_info()
            .map(|region| {
                source
                    .read_region(region.as_region())
                    .map(SessionInfoBuffer::from_owned_checked_region)
            })
            .transpose()?;

        source.seek_to_region_start(layout.frames().as_region())?;

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
            variable_schema,
            current_frame: 0,
            session_info,
        })
    }

    /// Returns the validated metadata and frame geometry.
    pub fn layout(&self) -> &IbtLayout {
        &self.layout
    }

    /// Reads an owned snapshot of exactly the advertised session region on each call.
    ///
    /// Returns `None` if absent. Does not change the logical replay position or
    /// refresh the legacy cached session information.
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
    /// Returns `None` if absent. Does not change the logical replay position or
    /// refresh the legacy cached schema.
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

    /// Reads exactly one indexed frame without changing the logical replay position.
    ///
    /// # Errors
    /// Returns an error if `index` is out of range or the complete frame cannot be read.
    pub fn frame(&mut self, index: usize) -> Result<Vec<u8>> {
        self.source
            .read_region(self.layout.frame(index)?.as_region())
    }

    /// Returns an owned snapshot of the file's advertised session-information region.
    ///
    /// Returns `None` when the header advertises no region. The snapshot is
    /// validated and cached during construction, so this method never reads or
    /// seeks the telemetry source.
    pub fn session_info_buffer(&self) -> Option<SessionInfoBuffer> {
        self.session_info.clone()
    }

    /// Returns decoded session-information text with invalid control characters removed.
    ///
    /// Returns `None` when the file has no session-information region or the
    /// NUL-bounded payload is empty after sanitization.
    pub fn session_yaml(&self) -> Option<String> {
        let buffer = self.session_info_buffer()?;
        let session_string = IRacingSessionString::try_from(buffer).ok()?;

        Some(session_string.into())
    }

    /// Get total number of frames in the file
    pub fn total_frames(&self) -> usize {
        self.layout.frame_count()
    }

    /// Get current frame position
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Get the tick rate from IBT header
    ///
    /// Returns the actual recording frequency, or 60Hz as fallback if invalid.
    pub fn tick_rate(&self) -> f64 {
        if self.header.tick_rate > 0 {
            self.header.tick_rate as f64
        } else {
            // Fallback to 60Hz if tick_rate is invalid
            60.0
        }
    }

    /// Get the current file location in seconds
    pub fn current_time(&self) -> f64 {
        self.current_frame() as f64 / self.tick_rate()
    }

    /// Get the total duration in seconds
    pub fn duration(&self) -> f64 {
        self.total_frames() as f64 / self.tick_rate()
    }

    /// Get disk metadata from the disk sub-header
    pub fn disk_header(&self) -> &DiskSubHeader {
        &self.disk_header
    }

    /// Get the IBT header information
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Positions the reader so the next call to [`Self::read_next_frame`] reads
    /// `frame_number`.
    ///
    /// # Errors
    ///
    /// Returns a parse error if `frame_number` is outside the file's frame range
    /// or its byte offset overflows the source address space.
    pub fn seek_to_frame(&mut self, frame_number: usize) -> Result<()> {
        let region = self.layout.frame(frame_number)?.as_region();
        self.source.seek_to_region_start(region)?;
        self.current_frame = frame_number;
        Ok(())
    }

    /// Reads the next frame as raw bytes and advances the reader by one frame.
    ///
    /// The returned tuple contains the frame data, its zero-based frame index as
    /// a synthetic tick, and the header's session-information update counter.
    /// Returns `Ok(None)` at end of file.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the source cannot supply the next complete
    /// advertised frame.
    pub fn read_next_frame(&mut self) -> Result<Option<(Vec<u8>, u32, u32)>> {
        // Check if we've reached the end
        if self.current_frame >= self.total_frames() {
            return Ok(None);
        }

        let frame_data = self.frame(self.current_frame)?;
        let tick_count = self.current_frame as u32;
        let session_version = self.header.session_info_update as u32;

        // Advance to next frame
        self.current_frame += 1;

        Ok(Some((frame_data, tick_count, session_version)))
    }
}

impl SchemaProvider for IbtReader {
    fn schema(&self) -> &VariableSchema {
        &self.variable_schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::{Context, Result, ensure};

    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

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
        let expected = IbtReader::from_bytes(original.clone())?.total_frames();
        for record_count in [-1, 0, 1, i32::MAX] {
            let mut bytes = original.clone();
            write_i32(&mut bytes, size_of::<Header>() + 28, record_count);
            let mut reader = IbtReader::from_bytes(bytes)?;
            assert_eq!(reader.total_frames(), expected);
            reader.seek_to_frame(expected - 1)?;
            assert!(reader.read_next_frame()?.is_some());
            assert!(reader.read_next_frame()?.is_none());
            assert!(reader.frame(expected).is_err());
        }
        Ok(())
    }

    #[test]
    fn seeking_beyond_the_final_frame_preserves_position() -> Result<()> {
        let mut reader = IbtReader::from_bytes(fixture_bytes()?)?;
        let position = reader.source.stream_position()?;
        assert!(reader.seek_to_frame(reader.total_frames()).is_err());
        assert_eq!(reader.current_frame(), 0);
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

        assert!(reader.read_next_frame().is_err());
        assert_eq!(reader.current_frame(), 0);
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

        let reader = IbtReader::from_bytes(data)?;

        assert_eq!(reader.current_frame(), 0);
        assert!(reader.total_frames() > 0);
        assert!(reader.variable_count() > 0);
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

        let (frame, _, _) = reader
            .read_next_frame()?
            .context("fixture should contain a frame")?;
        assert_eq!(frame[0], replacement);
        Ok(())
    }

    #[test]
    fn metadata_remains_owned_after_the_source_changes() -> Result<()> {
        let temporary_directory = tempfile::tempdir()?;
        let temporary_file = temporary_directory.path().join("cached-metadata.ibt");
        std::fs::copy(fixture_path()?, &temporary_file)?;
        let reader = IbtReader::open(&temporary_file)?;
        let expected_yaml = reader
            .session_yaml()
            .context("fixture should contain session metadata")?;

        let mut file = OpenOptions::new().write(true).open(&temporary_file)?;
        file.seek(SeekFrom::Start(u64::try_from(
            reader.header().session_info_offset,
        )?))?;
        let session_length = usize::try_from(reader.header().session_info_length)?;
        file.write_all(&vec![0; session_length])?;
        file.flush()?;

        assert_eq!(
            reader.session_yaml().as_deref(),
            Some(expected_yaml.as_str())
        );
        Ok(())
    }

    #[test]
    fn source_cursor_tracks_the_next_logical_frame() -> Result<()> {
        let mut reader = IbtReader::open(fixture_path()?)?;
        assert_source_cursor(&mut reader)?;

        reader
            .read_next_frame()?
            .context("fixture should contain a first frame")?;
        assert_source_cursor(&mut reader)?;

        let target = reader.total_frames() / 2;
        reader.seek_to_frame(target)?;
        assert_source_cursor(&mut reader)?;

        let position_before_metadata = reader.source.stream_position()?;
        reader
            .session_yaml()
            .context("fixture should contain session metadata")?;
        assert_eq!(reader.source.stream_position()?, position_before_metadata);

        reader
            .read_next_frame()?
            .context("fixture should contain the sought frame")?;
        assert_source_cursor(&mut reader)?;
        Ok(())
    }

    fn assert_source_cursor(reader: &mut IbtReader) -> Result<()> {
        let frame = u64::try_from(reader.current_frame)?;
        let frame_size = u64::try_from(reader.layout.frame_size())?;
        let expected = u64::try_from(reader.layout.frame_data_start())? + frame * frame_size;
        assert_eq!(reader.source.stream_position()?, expected);
        Ok(())
    }

    #[test]
    fn test_real_ibt_reader_construction() -> Result<()> {
        let test_file = fixture_path()?;
        println!("Testing reader construction with: {}", test_file.display());

        let reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        println!("Reader constructed successfully:");
        println!("  Total frames: {}", reader.total_frames());
        println!("  Current frame: {}", reader.current_frame());

        assert_eq!(reader.current_frame(), 0, "Should start at frame 0");

        if reader.total_frames() == 0 {
            println!("  This IBT file contains only session info (no telemetry data)");
        } else {
            println!(
                "  This IBT file contains {} frames of telemetry data",
                reader.total_frames()
            );
        }

        Ok(())
    }

    #[test]
    fn test_real_ibt_read_next_frame() -> Result<()> {
        let test_file = fixture_path()?;
        let mut reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        let total_frames = reader.total_frames();
        println!("IBT file has {} total frames", total_frames);

        if total_frames == 0 {
            println!(
                "Fixture {} contains no telemetry frames; skipping frame validation",
                test_file.display()
            );
            return Ok(());
        }

        let first = reader
            .read_next_frame()
            .with_context(|| format!("Reading first frame from {}", test_file.display()))?;
        let (data, tick_count, _session_version) =
            first.expect("IBT fixtures should yield at least one frame");

        ensure!(
            !data.is_empty(),
            "Expected non-empty frame data from {}",
            test_file.display()
        );
        ensure!(
            data.len() == reader.schema().frame_size,
            "Frame data length {} must match schema frame size {}",
            data.len(),
            reader.schema().frame_size
        );
        ensure!(
            reader.variable_count() > 0,
            "Schema should expose telemetry variables"
        );
        ensure!(
            tick_count == 0,
            "First frame should have tick_count = 0, but got {} from {}",
            tick_count,
            test_file.display()
        );
        ensure!(
            reader.current_frame() == 1,
            "Reader should advance to frame index 1 after consuming the first frame"
        );

        Ok(())
    }

    #[test]
    fn test_real_ibt_end_of_file_handling() -> Result<()> {
        let test_file = fixture_path()?;
        let mut reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        let total_frames = reader.total_frames();
        if total_frames == 0 {
            println!(
                "Fixture {} contains session info only; skipping EOF handling test",
                test_file.display()
            );
            return Ok(());
        }

        let last_index = total_frames - 1;
        reader.seek_to_frame(last_index).with_context(|| {
            format!(
                "Seeking to final frame {} in {}",
                last_index,
                test_file.display()
            )
        })?;

        let last = reader
            .read_next_frame()
            .with_context(|| format!("Reading final frame from {}", test_file.display()))?;
        let (_, tick_count, _) = last.expect("Expected frame data after seeking to final frame");

        ensure!(
            tick_count as usize == last_index,
            "Final frame tick {} should match requested index {}",
            tick_count,
            last_index
        );

        let eof = reader
            .read_next_frame()
            .with_context(|| format!("Reading EOF sentinel from {}", test_file.display()))?;
        ensure!(
            eof.is_none(),
            "read_next_frame should return None once EOF is reached"
        );

        Ok(())
    }

    #[test]
    fn test_real_ibt_frame_seeking() -> Result<()> {
        let test_file = fixture_path()?;
        let mut reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        let total_frames = reader.total_frames();
        if total_frames < 3 {
            println!(
                "Fixture {} has {} frames; skipping seek test that requires at least 3",
                test_file.display(),
                total_frames
            );
            return Ok(());
        }

        let middle = total_frames / 2;
        reader
            .seek_to_frame(middle)
            .context("Seeking to middle frame")?;

        let frame = reader
            .read_next_frame()
            .context("Reading frame after seek")?
            .expect("Expected frame after seeking to target index");
        let (_, tick_count, _) = frame;

        ensure!(
            tick_count as usize == middle,
            "Frame tick {} should match requested index {}",
            tick_count,
            middle
        );

        Ok(())
    }

    #[test]
    fn test_real_ibt_read_next_frame_performance() -> Result<()> {
        let test_file = fixture_path()?;
        let mut reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        if reader.total_frames() == 0 {
            println!(
                "Fixture {} contains no frames; skipping latency measurement",
                test_file.display()
            );
            return Ok(());
        }

        let start = Instant::now();
        let frame = reader
            .read_next_frame()
            .context("Reading frame to measure latency")?;
        let elapsed = start.elapsed();

        ensure!(
            frame.is_some(),
            "Expected frame data on first call to read_next_frame()"
        );
        ensure!(
            elapsed < Duration::from_millis(100),
            "Frame retrieval should be fast (took {:?})",
            elapsed
        );

        Ok(())
    }

    #[test]
    fn test_real_ibt_raw_frame_validation() -> Result<()> {
        let test_file = fixture_path()?;
        let mut reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        if reader.total_frames() == 0 {
            println!(
                "Fixture {} contains no frames; skipping raw frame validation",
                test_file.display()
            );
            return Ok(());
        }

        let frame = reader
            .read_next_frame()
            .with_context(|| format!("Reading frame for validation from {}", test_file.display()))?
            .expect("Expected frame for validation");
        let (data, _, _) = frame;
        let schema = reader.schema();

        ensure!(
            schema.variable_count() > 0,
            "Schema should contain telemetry variables"
        );
        ensure!(
            schema.has_variable("SessionTime"),
            "Schema should expose SessionTime variable"
        );
        ensure!(
            schema.frame_size == data.len(),
            "Schema frame size {} must match data length {}",
            schema.frame_size,
            data.len()
        );

        if let Some(speed) = schema.get_variable("Speed") {
            ensure!(
                speed.offset + speed.data_type.byte_size() * speed.count <= data.len(),
                "Speed variable must fit within the frame buffer"
            );
        }

        Ok(())
    }

    #[test]
    fn test_real_ibt_session_yaml_extraction() -> Result<()> {
        let test_file = fixture_path()?;
        let reader = IbtReader::open(&test_file)
            .with_context(|| format!("Opening {}", test_file.display()))?;

        println!(
            "Testing session YAML extraction from {}",
            test_file.display()
        );

        // Extract session YAML
        let yaml = reader
            .session_yaml()
            .with_context(|| "Extracting session YAML")?;

        // Verify YAML is non-empty
        ensure!(!yaml.is_empty(), "Session YAML should not be empty");

        println!("  Session YAML extracted: {} bytes", yaml.len());

        // Verify YAML structure - should contain expected top-level keys
        ensure!(
            yaml.contains("WeekendInfo:"),
            "YAML should contain WeekendInfo section"
        );
        ensure!(
            yaml.contains("SessionInfo:"),
            "YAML should contain SessionInfo section"
        );

        // Verify the YAML has been preprocessed (no control characters)
        for (i, ch) in yaml.chars().enumerate() {
            if matches!(ch, '\x00'..='\x08' | '\x0B'..='\x0C' | '\x0E'..='\x1F') {
                anyhow::bail!(
                    "Found control character 0x{:02X} at position {} - YAML not properly preprocessed",
                    ch as u8,
                    i
                );
            }
        }

        // Verify the YAML can be parsed into SessionInfo
        let session = crate::schema::SessionInfo::parse(&yaml)
            .with_context(|| "Parsing extracted YAML into SessionInfo")?;

        println!("  Track: {}", session.weekend_info.track_name);
        println!("  Sessions: {}", session.session_info.sessions.len());

        // Verify basic session info structure
        ensure!(
            !session.weekend_info.track_name.is_empty(),
            "Track name should not be empty"
        );
        ensure!(
            !session.session_info.sessions.is_empty(),
            "Should have at least one session"
        );

        Ok(())
    }
    #[test]
    fn generated_fixture_metadata_matches_manifest() -> Result<()> {
        let manifest = crate::test_utils::load_fixture_manifest()?;

        for fixture in &manifest.fixtures {
            let file_path = fixture.fixture_path()?;
            let reader = crate::ibt::IbtReader::open(&file_path)
                .with_context(|| format!("Opening {}", file_path.display()))?;
            ensure!(
                reader.total_frames() > 0,
                "Fixture should contain telemetry frames"
            );
            assert_eq!(reader.total_frames(), fixture.num_frames);
            assert_eq!(reader.tick_rate(), fixture.tick_rate as f64);
        }

        Ok(())
    }
}
