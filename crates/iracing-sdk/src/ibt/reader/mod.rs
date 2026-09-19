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

use super::format::extract_variable_schema;
use crate::{
    ByteRegion, IRacingSDKError, Result, SchemaProvider, SessionInfoBuffer, VariableHeaderRegion,
    VariableSchema,
    irsdk::{DiskSubHeader, Header},
    types::{IRacingSessionString, SessionInfoRegion},
};
use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
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

/// IBT file reader for cross-platform replay.
///
/// After construction and between successful frame operations, the source
/// cursor is positioned at
/// `frame_data_start + current_frame * frame_size`. Metadata access uses owned
/// snapshots and does not disturb that cursor.
pub struct IbtReader {
    source: IbtSource,
    path: Option<PathBuf>,

    header: Header,
    disk_header: DiskSubHeader,
    variable_schema: VariableSchema,

    session_info: Option<SessionInfoBuffer>,
    // variable_headers_region: VariableHeaderRegion,
    current_frame: usize,
    total_frames: usize,
    frame_data_start: u64,
    frame_size: usize,
}

impl IbtReader {
    /// Open and parse an `.ibt` file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;

        Self::from_source(IbtSource::File(file), Some(path))
    }

    /// Parse owned in-memory `.ibt` data.
    pub fn from_bytes<B: Into<Vec<u8>>>(data: B) -> Result<Self> {
        Self::from_source(IbtSource::Memory(Cursor::new(data.into())), None)
    }

    fn from_source(mut source: IbtSource, path: Option<PathBuf>) -> Result<Self> {
        let source_len = source.seek(SeekFrom::End(0)).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT source length",
                format!("Failed to determine source length: {error}"),
            )
        })?;
        source.seek(SeekFrom::Start(0)).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT source seek",
                format!("Failed to seek to source start: {error}"),
            )
        })?;

        // Parse IBT header
        let header = Header::try_from_reader(&mut source)?;
        header.validate_ibt()?;

        // Parse disk sub-header (note: may be corrupted, but we'll try)
        let disk_header = DiskSubHeader::try_from_reader(&mut source)?;

        let variable_headers_region = VariableHeaderRegion::try_from(&header)?;
        let variable_headers_end =
            Self::validate_region(variable_headers_region.as_region(), source_len)?;
        let frame_size = usize::try_from(header.buffer_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "Variable headers parse",
                "Could not parse buffer_length to usize",
            )
        })?;

        // Extract variable schema
        let variable_schema =
            extract_variable_schema(&mut source, &variable_headers_region, frame_size)?;

        let session_info_region = SessionInfoRegion::try_from(&header)?;

        // IBT does not advertise a separate telemetry-region length. Preserve
        // the offset-based parser contract by starting after the latest present
        // metadata region and deriving complete frames from physical EOF.
        let frame_data_start = if session_info_region.is_valid() {
            let session_info_end =
                Self::validate_region(session_info_region.as_region(), source_len)?;
            session_info_end.max(variable_headers_end)
        } else {
            variable_headers_end
        };

        let session_info = if session_info_region.is_valid() {
            let bytes =
                Self::read_owned_region(&mut source, source_len, session_info_region.as_region())?;
            Some(SessionInfoBuffer::from_owned_checked_region(bytes))
        } else {
            None
        };

        source
            .seek(SeekFrom::Start(frame_data_start))
            .map_err(|error| {
                IRacingSDKError::parse_error(
                    "Frame data seek",
                    format!("Failed to seek to first frame at {frame_data_start}: {error}"),
                )
            })?;

        // Calculate total frames based on remaining file data with bounds checking
        let remaining_bytes = source_len.checked_sub(frame_data_start).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "Frame data calculation",
                "Frame data start position exceeds file size",
            )
        })?;
        let frame_size_u64 = u64::try_from(frame_size).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame data calculation",
                "Frame size cannot be represented as a source offset",
            )
        })?;
        let total_frames = remaining_bytes.checked_div(frame_size_u64).unwrap_or(0);
        let total_frames = usize::try_from(total_frames).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame data calculation",
                "Frame count cannot be represented as usize",
            )
        })?;

        // The writer's record count is advisory for bounds: incomplete files
        // and stale headers can disagree with physical EOF. Never let it
        // authorize reading outside the EOF-derived complete-frame region.
        if disk_header.record_count > 0 && total_frames > 0 {
            let expected_frames = disk_header.record_count as usize;
            if expected_frames != total_frames {
                tracing::warn!(
                    "Frame count mismatch: disk header reports {} records, calculated {} frames from file size",
                    disk_header.record_count,
                    total_frames
                );
            }
        }

        Ok(IbtReader {
            source,
            path,
            header,
            disk_header,
            variable_schema,
            current_frame: 0,
            total_frames,
            frame_data_start,
            frame_size,
            // variable_headers_region,
            session_info,
        })
    }

    fn validate_region(region: ByteRegion, source_len: u64) -> Result<u64> {
        let range = region.as_checked_range()?;
        let end = u64::try_from(range.end).map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT region bounds",
                "Region end cannot be represented as a source offset",
            )
        })?;
        if end > source_len {
            return Err(IRacingSDKError::parse_error(
                "IBT region bounds",
                format!(
                    "Region {}..{} exceeds source length {source_len}",
                    range.start, range.end
                ),
            ));
        }
        Ok(end)
    }

    fn read_owned_region(
        source: &mut IbtSource,
        source_len: u64,
        region: ByteRegion,
    ) -> Result<Vec<u8>> {
        Self::validate_region(region, source_len)?;
        let offset = u64::try_from(region.offset).map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT region read",
                "Region offset cannot be represented as a source offset",
            )
        })?;
        source.seek(SeekFrom::Start(offset)).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT region seek",
                format!("Failed to seek to offset {offset}: {error}"),
            )
        })?;
        let mut bytes = vec![0; region.length];
        source.read_exact(&mut bytes).map_err(|error| {
            IRacingSDKError::parse_error(
                "IBT region read",
                format!(
                    "Failed to read {} bytes at offset {offset}: {error}",
                    region.length
                ),
            )
        })?;
        Ok(bytes)
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
        self.total_frames
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

    /// Get the file path this reader was opened from, if it has one.
    ///
    /// Readers constructed with [`Self::from_bytes`] have no filesystem path.
    pub fn file_path(&self) -> Option<&Path> {
        self.path.as_deref()
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
        if frame_number >= self.total_frames {
            return Err(IRacingSDKError::Parse {
                context: "Frame seek".to_string(),
                details: format!(
                    "Frame {} out of range (0..{})",
                    frame_number, self.total_frames
                ),
            });
        }

        // Calculate position for frame with checked arithmetic
        let requested_frame = frame_number;
        let frame_size = u64::try_from(self.frame_size).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame seek",
                "Frame size cannot be represented as a source offset",
            )
        })?;
        let frame_number = u64::try_from(frame_number).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame seek",
                "Frame number cannot be represented as a source offset",
            )
        })?;
        let frame_byte_offset = frame_number.checked_mul(frame_size).ok_or_else(|| {
            IRacingSDKError::parse_error("Frame seek", "Frame offset calculation overflowed")
        })?;

        let frame_offset = self
            .frame_data_start
            .checked_add(frame_byte_offset)
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Frame seek".to_string(),
                details: "Frame position calculation overflowed".to_string(),
            })?;

        self.source
            .seek(SeekFrom::Start(frame_offset))
            .map_err(|error| {
                IRacingSDKError::parse_error(
                    "Frame seek",
                    format!("Failed to seek to frame {requested_frame}: {error}"),
                )
            })?;
        self.current_frame = requested_frame;
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
    /// Returns a parse error if the next advertised frame extends beyond the
    /// loaded file data.
    pub fn read_next_frame(&mut self) -> Result<Option<(Vec<u8>, u32, u32)>> {
        // Check if we've reached the end
        if self.current_frame >= self.total_frames {
            return Ok(None);
        }

        let frame_size_u64 = u64::try_from(self.frame_size).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame reading",
                "Frame size cannot be represented as a source offset",
            )
        })?;
        let frame_number = u64::try_from(self.current_frame).map_err(|_| {
            IRacingSDKError::parse_error(
                "Frame reading",
                "Frame number cannot be represented as a source offset",
            )
        })?;
        let frame_offset = frame_number
            .checked_mul(frame_size_u64)
            .and_then(|offset| self.frame_data_start.checked_add(offset))
            .ok_or_else(|| {
                IRacingSDKError::parse_error("Frame reading", "Frame position overflowed")
            })?;
        let mut frame_data = vec![0; self.frame_size];
        if let Err(error) = self.source.read_exact(&mut frame_data) {
            let _ = self.source.seek(SeekFrom::Start(frame_offset));
            return Err(IRacingSDKError::parse_error(
                "Frame reading",
                format!("Failed to read frame {}: {error}", self.current_frame),
            ));
        }
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

    #[test]
    fn from_bytes_builds_an_owned_memory_reader() -> Result<()> {
        let test_file = fixture_path()?;
        let data = std::fs::read(test_file)?;

        let reader = IbtReader::from_bytes(data)?;

        assert_eq!(reader.file_path(), None);
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
        file.seek(SeekFrom::Start(reader.frame_data_start))?;
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
        let frame_size = u64::try_from(reader.frame_size)?;
        let expected = reader.frame_data_start + frame * frame_size;
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
        assert_eq!(reader.file_path(), Some(test_file.as_path()));

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
                speed.offset + speed.data_type.byte_size().unwrap() * speed.count <= data.len(),
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
