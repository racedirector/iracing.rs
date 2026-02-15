//! IBT file reader for telemetry replay
//!
//! Provides cross-platform IBT file reading for use with the ReplayProvider.
//! This allows IBT files to be replayed through the same architecture as live telemetry.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use pitwall::ibt::IbtReader;
//!
//! fn read_frames() -> pitwall::Result<()> {
//!     // Open IBT file
//!     let mut reader = IbtReader::open("telemetry.ibt")?;
//!     println!("File contains {} frames", reader.total_frames());
//!
//!     // Read frames sequentially
//!     while let Some((frame_data, tick, session_version)) = reader.read_next_frame()? {
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
//! - File data is loaded into memory at construction time for fast random access
//! - Frame reading is zero-allocation except for the returned `RawFrame`
//! - Seeking operations are O(1) as they only update internal position counters

use super::format::{IRSDK_VAR_HEADER_SIZE, IbtDiskSubHeader, IbtHeader, extract_variable_schema};
use crate::{Result, TelemetryError, VariableSchema, yaml_utils};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::warn;

/// IBT file reader that implements FrameProvider for cross-platform replay
pub struct IbtReader {
    data: Vec<u8>,
    current_position: usize,
    path: PathBuf,
    header: IbtHeader,
    disk_header: IbtDiskSubHeader,
    variable_schema: VariableSchema,
    current_frame: usize,
    total_frames: usize,
    frame_data_start: usize,
}

impl IbtReader {
    /// Open an IBT file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(&path)
            .map_err(|e| TelemetryError::File { path: path.as_ref().to_path_buf(), source: e })?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| TelemetryError::File { path: path.as_ref().to_path_buf(), source: e })?;

        Self::from_bytes_with_path(&data, path.as_ref().to_path_buf())
    }

    /// Create IbtReader from bytes (for testing)
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with_path(data, PathBuf::from("<memory>"))
    }

    /// Create IbtReader from bytes with path context
    fn from_bytes_with_path(data: &[u8], path: PathBuf) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        // Parse IBT header
        let header = IbtHeader::parse_from_reader(&mut cursor)?;
        header.validate()?;

        // Parse disk sub-header (note: may be corrupted, but we'll try)
        let disk_header = IbtDiskSubHeader::parse_from_reader(&mut cursor)?;

        // Extract variable schema
        let variable_schema = extract_variable_schema(&mut cursor, &header)?;

        // Calculate frame data start position correctly with checked arithmetic
        // Frame data starts AFTER both variable headers AND session info
        // 1. Variable headers are at header.var_header_offset and each is IRSDK_VAR_HEADER_SIZE bytes
        let var_headers_size = header
            .num_vars
            .checked_mul(IRSDK_VAR_HEADER_SIZE as i32)
            .ok_or_else(|| TelemetryError::Parse {
                context: "Frame data calculation".to_string(),
                details: "Variable headers size calculation overflowed".to_string(),
            })?;

        let var_headers_end =
            header.var_header_offset.checked_add(var_headers_size).ok_or_else(|| {
                TelemetryError::Parse {
                    context: "Frame data calculation".to_string(),
                    details: "Variable headers end calculation overflowed".to_string(),
                }
            })?;

        // 2. Session info comes after variable headers (if present)
        let session_info_end = if header.session_info_len > 0 {
            header.session_info_offset.checked_add(header.session_info_len).ok_or_else(|| {
                TelemetryError::Parse {
                    context: "Frame data calculation".to_string(),
                    details: "Session info end calculation overflowed".to_string(),
                }
            })?
        } else {
            var_headers_end
        };

        // Frame data starts after whichever comes last: variable headers or session info
        let frame_data_start = session_info_end.max(var_headers_end) as usize;

        // Calculate total frames based on remaining file data with bounds checking
        let remaining_bytes =
            data.len().checked_sub(frame_data_start).ok_or_else(|| TelemetryError::Parse {
                context: "Frame data calculation".to_string(),
                details: "Frame data start position exceeds file size".to_string(),
            })?;

        let total_frames = if header.buf_len > 0 {
            remaining_bytes / header.buf_len as usize
        } else {
            0 // No telemetry data if buf_len is 0
        };

        // Cross-check disk_header.record_count against total_frames for debugging
        if disk_header.record_count > 0 && total_frames > 0 {
            let expected_frames = disk_header.record_count as usize;
            if expected_frames != total_frames {
                warn!(
                    "Frame count mismatch: disk header reports {} records, calculated {} frames from file size",
                    disk_header.record_count, total_frames
                );
            }
        }

        let reader = IbtReader {
            data: data.to_vec(),
            current_position: frame_data_start,
            path,
            header,
            disk_header,
            variable_schema,
            current_frame: 0,
            total_frames,
            frame_data_start,
        };

        Ok(reader)
    }

    /// Get cleaned session YAML from the IBT file
    ///
    /// Returns preprocessed YAML string ready for parsing. The YAML has been cleaned
    /// to fix iRacing's non-standard format issues. Parsing happens at the Connection level.
    /// This method extracts on-demand, no caching.
    pub fn session_yaml(&self) -> Result<Option<String>> {
        // Check if session info exists
        if self.header.session_info_len <= 0 || self.header.session_info_offset <= 0 {
            return Ok(None);
        }

        // Extract raw YAML from memory
        let raw_yaml = yaml_utils::extract_yaml_from_memory(
            &self.data,
            self.header.session_info_offset,
            self.header.session_info_len,
        )?;

        // Return None if empty
        if raw_yaml.trim().is_empty() {
            return Ok(None);
        }

        // Preprocess to fix iRacing's YAML issues
        let cleaned_yaml = yaml_utils::preprocess_iracing_yaml(&raw_yaml)?;

        Ok(Some(cleaned_yaml))
    }

    /// Get the variable schema for this IBT file
    pub fn variables(&self) -> &VariableSchema {
        &self.variable_schema
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

    /// Get the file path this reader was opened from
    pub fn file_path(&self) -> &Path {
        &self.path
    }

    /// Get disk metadata from the disk sub-header
    pub fn disk_header(&self) -> &IbtDiskSubHeader {
        &self.disk_header
    }

    /// Get the IBT header information
    pub fn header(&self) -> &IbtHeader {
        &self.header
    }

    /// Seek to a specific frame (for random access)
    pub fn seek_to_frame(&mut self, frame_number: usize) -> Result<()> {
        if frame_number >= self.total_frames {
            return Err(TelemetryError::Parse {
                context: "Frame seek".to_string(),
                details: format!("Frame {} out of range (0..{})", frame_number, self.total_frames),
            });
        }

        // Calculate position for frame with checked arithmetic
        let frame_size = self.header.buf_len as usize;
        let frame_byte_offset =
            frame_number.checked_mul(frame_size).ok_or_else(|| TelemetryError::Parse {
                context: "Frame seek".to_string(),
                details: "Frame offset calculation overflowed".to_string(),
            })?;

        let frame_offset =
            self.frame_data_start.checked_add(frame_byte_offset).ok_or_else(|| {
                TelemetryError::Parse {
                    context: "Frame seek".to_string(),
                    details: "Frame position calculation overflowed".to_string(),
                }
            })?;

        self.current_position = frame_offset;
        self.current_frame = frame_number;
        Ok(())
    }

    /// Read the next frame as raw bytes
    ///
    /// Returns frame data, tick count, and session version for FramePacket construction
    pub fn read_next_frame(&mut self) -> Result<Option<(Vec<u8>, u32, u32)>> {
        // Check if we've reached the end
        if self.current_frame >= self.total_frames {
            return Ok(None);
        }

        // Handle IBT files with no telemetry data
        if self.header.buf_len == 0 {
            return Ok(None);
        }

        let frame_size = self.header.buf_len as usize;
        let start_pos = self.current_position;
        let end_pos = start_pos + frame_size;

        if end_pos > self.data.len() {
            return Err(TelemetryError::Parse {
                context: "Frame reading".to_string(),
                details: format!(
                    "Frame {} extends beyond data bounds ({} > {})",
                    self.current_frame,
                    end_pos,
                    self.data.len()
                ),
            });
        }

        let frame_data = self.data[start_pos..end_pos].to_vec();
        let tick_count = self.current_frame as u32;
        let session_version = self.header.session_info_update as u32;

        // Advance to next frame
        self.current_frame += 1;
        self.current_position = end_pos;

        Ok(Some((frame_data, tick_count, session_version)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::{Context, Result, ensure};

    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn fixture_path() -> Result<PathBuf> {
        Ok(require_smallest_ibt_fixture()?)
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
            println!("  This IBT file contains {} frames of telemetry data", reader.total_frames());
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

        ensure!(!data.is_empty(), "Expected non-empty frame data from {}", test_file.display());
        ensure!(
            data.len() == reader.variables().frame_size,
            "Frame data length {} must match schema frame size {}",
            data.len(),
            reader.variables().frame_size
        );
        ensure!(
            reader.variables().variable_count() > 0,
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
            format!("Seeking to final frame {} in {}", last_index, test_file.display())
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
        ensure!(eof.is_none(), "read_next_frame should return None once EOF is reached");

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
        reader.seek_to_frame(middle).context("Seeking to middle frame")?;

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
        let frame = reader.read_next_frame().context("Reading frame to measure latency")?;
        let elapsed = start.elapsed();

        ensure!(frame.is_some(), "Expected frame data on first call to read_next_frame()");
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
        let schema = reader.variables();

        ensure!(schema.variable_count() > 0, "Schema should contain telemetry variables");
        ensure!(schema.has_variable("SessionTime"), "Schema should expose SessionTime variable");
        ensure!(
            schema.frame_size == data.len(),
            "Schema frame size {} must match data length {}",
            schema.frame_size,
            data.len()
        );

        if let Some(speed) = schema.get_variable("Speed") {
            ensure!(
                speed.offset + speed.data_type.size() * speed.count <= data.len(),
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

        println!("Testing session YAML extraction from {}", test_file.display());

        // Extract session YAML
        let yaml_result = reader.session_yaml().with_context(|| "Extracting session YAML")?;

        // Verify we got YAML
        let yaml = yaml_result.expect("IBT file should contain session YAML");

        // Verify YAML is non-empty
        ensure!(!yaml.is_empty(), "Session YAML should not be empty");

        println!("  Session YAML extracted: {} bytes", yaml.len());

        // Verify YAML structure - should contain expected top-level keys
        ensure!(yaml.contains("WeekendInfo:"), "YAML should contain WeekendInfo section");
        ensure!(yaml.contains("SessionInfo:"), "YAML should contain SessionInfo section");

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
        let session = crate::SessionInfo::parse(&yaml)
            .with_context(|| "Parsing extracted YAML into SessionInfo")?;

        println!("  Track: {}", session.weekend_info.track_name);
        println!("  Sessions: {}", session.session_info.sessions.len());

        // Verify basic session info structure
        ensure!(!session.weekend_info.track_name.is_empty(), "Track name should not be empty");
        ensure!(!session.session_info.sessions.is_empty(), "Should have at least one session");

        Ok(())
    }
}
