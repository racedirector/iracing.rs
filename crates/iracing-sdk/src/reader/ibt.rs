//! Immutable IBT storage, checked frame layout, and cursor policy.
//!
//! [`IbtRecording`] parses and validates the immutable portions of an IBT once:
//! the common header, disk sub-header, advertised metadata regions, first frame
//! offset, frame length, and record count. It owns no mutable cursor and offers
//! checked random frame access.
//!
//! [`IbtReader`] layers one sequential cursor over a recording. Seeking changes
//! only the next frame index; byte offsets are derived when a frame is read, so
//! there is no second position counter that can drift out of sync.
//!
//! This module returns wire snapshots. Schema construction, YAML preprocessing,
//! synthetic tick assignment, and provider delivery policy remain outside the
//! reader.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use crate::{
    IRacingSDKError, Result,
    irsdk::{DiskSubHeader, FrameBuffer, Header, SessionInfoBuffer, VariableHeadersBuffer},
    types::IbtHeader,
};

use super::{
    access_source::{ByteRegion, OwnedBytes, RandomAccessSource},
    header::{HeaderRegions, HeaderSnapshotReader},
};

/// Immutable parsed structure of one complete IBT source.
#[derive(Debug, Clone)]
pub struct IbtRecording<S = OwnedBytes> {
    source: S,
    header: IbtHeader,
    regions: HeaderRegions,
    frame_data_start: usize,
    total_frames: usize,
}

impl IbtRecording<OwnedBytes> {
    /// Parses owned IBT bytes into an immutable recording.
    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Self::from_source(OwnedBytes::from(bytes.into()))
    }

    /// Materializes and parses a sequential IBT source.
    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        Self::from_source(OwnedBytes::from_reader(reader)?)
    }

    /// Opens, materializes, and parses an IBT file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path).map_err(|source| IRacingSDKError::File {
            path: path.clone(),
            source,
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|source| IRacingSDKError::File {
                path: path.clone(),
                source,
            })?;

        Self::from_bytes(bytes)
    }
}

impl<S: RandomAccessSource> IbtRecording<S> {
    /// Parses an arbitrary finite random-access source.
    pub fn from_source(source: S) -> Result<Self> {
        let header_bytes = source.snapshot(ByteRegion::new(0, IbtHeader::SIZE)?)?;
        let header_buffer: &[u8; IbtHeader::SIZE] =
            header_bytes.as_slice().try_into().map_err(|_| {
                IRacingSDKError::parse_error(
                    "IBT header",
                    format!("Expected exactly {} header bytes", IbtHeader::SIZE),
                )
            })?;
        let header = IbtHeader::try_from_buffer(header_buffer)?;
        header.validate()?;

        let regions = HeaderRegions::from_header(&header.header)?;
        validate_metadata_regions(&source, regions)?;
        let frame_data_start = frame_data_start(regions);
        let total_frames = validate_frame_layout(
            source.len(),
            frame_data_start,
            regions.frame_length(),
            header.record_count(),
        )?;

        Ok(Self {
            source,
            header,
            regions,
            frame_data_start,
            total_frames,
        })
    }

    /// Returns the parsed common and disk headers.
    pub fn ibt_header(&self) -> &IbtHeader {
        &self.header
    }

    /// Returns the common SDK header.
    pub fn header(&self) -> &Header {
        &self.header.header
    }

    /// Returns the IBT-specific disk sub-header.
    pub fn disk_header(&self) -> &DiskSubHeader {
        &self.header.sub_header
    }

    /// Returns the interpreted common-header regions.
    pub fn regions(&self) -> HeaderRegions {
        self.regions
    }

    /// Returns the absolute offset of the first telemetry frame.
    pub fn frame_data_start(&self) -> usize {
        self.frame_data_start
    }

    /// Returns the exact byte length of each telemetry frame.
    pub fn frame_length(&self) -> usize {
        self.regions.frame_length()
    }

    /// Returns the validated number of complete telemetry frames.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Copies the immutable session-information region.
    pub fn session_info_buffer(&self) -> Result<Option<SessionInfoBuffer>> {
        HeaderSnapshotReader::new(&self.source, self.header())?.session_info_buffer()
    }

    /// Copies the immutable variable-header array.
    pub fn variable_headers_buffer(&self) -> Result<Option<VariableHeadersBuffer>> {
        HeaderSnapshotReader::new(&self.source, self.header())?.variable_headers_buffer()
    }

    /// Copies a frame by zero-based index.
    ///
    /// Returns `Ok(None)` when `frame_index` is outside the recording.
    pub fn frame(&self, frame_index: usize) -> Result<Option<IbtFrame>> {
        if frame_index >= self.total_frames {
            return Ok(None);
        }

        let frame_offset = frame_index
            .checked_mul(self.frame_length())
            .and_then(|relative| self.frame_data_start.checked_add(relative))
            .ok_or_else(|| {
                IRacingSDKError::parse_error("IBT frame", "Frame byte offset overflowed")
            })?;
        let buffer =
            HeaderSnapshotReader::new(&self.source, self.header())?.frame_at(frame_offset)?;

        Ok(Some(IbtFrame {
            index: frame_index,
            buffer,
        }))
    }
}

/// One indexed frame copied from an immutable IBT recording.
#[derive(Debug, Clone)]
pub struct IbtFrame {
    index: usize,
    buffer: FrameBuffer,
}

impl IbtFrame {
    /// Returns the zero-based frame index within the recording.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the owned wire snapshot.
    pub fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }

    /// Releases the owned wire snapshot.
    pub fn into_buffer(self) -> FrameBuffer {
        self.buffer
    }
}

/// Sequential cursor over an immutable [`IbtRecording`].
#[derive(Debug, Clone)]
pub struct IbtReader<S = OwnedBytes> {
    recording: IbtRecording<S>,
    next_frame: usize,
    path: Option<PathBuf>,
}

impl IbtReader<OwnedBytes> {
    /// Opens an IBT file with filesystem-origin metadata.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let recording = IbtRecording::open(&path)?;
        Ok(Self {
            recording,
            next_frame: 0,
            path: Some(path),
        })
    }

    /// Parses owned in-memory IBT bytes.
    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Ok(Self::from_recording(IbtRecording::from_bytes(bytes)?))
    }

    /// Materializes and parses a sequential IBT source.
    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        Ok(Self::from_recording(IbtRecording::from_reader(reader)?))
    }
}

impl<S: RandomAccessSource> IbtReader<S> {
    /// Creates a cursor positioned at the first frame of a recording.
    pub fn from_recording(recording: IbtRecording<S>) -> Self {
        Self {
            recording,
            next_frame: 0,
            path: None,
        }
    }

    /// Returns the immutable recording behind this cursor.
    pub fn recording(&self) -> &IbtRecording<S> {
        &self.recording
    }

    /// Returns the filesystem path used by [`Self::open`], when available.
    pub fn file_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the index of the next frame that will be read.
    pub fn current_frame(&self) -> usize {
        self.next_frame
    }

    /// Returns the validated number of frames in the recording.
    pub fn total_frames(&self) -> usize {
        self.recording.total_frames()
    }

    /// Returns the common SDK header.
    pub fn header(&self) -> &Header {
        self.recording.header()
    }

    /// Returns the IBT-specific disk sub-header.
    pub fn disk_header(&self) -> &DiskSubHeader {
        self.recording.disk_header()
    }

    /// Returns the recording frequency in ticks per second.
    pub fn tick_rate(&self) -> f64 {
        self.header().tick_rate as f64
    }

    /// Returns the playback position represented by the cursor.
    pub fn current_time(&self) -> f64 {
        self.current_frame() as f64 / self.tick_rate()
    }

    /// Returns the duration represented by all validated frames.
    pub fn duration(&self) -> f64 {
        self.total_frames() as f64 / self.tick_rate()
    }

    /// Copies the immutable session-information region.
    pub fn session_info_buffer(&self) -> Result<Option<SessionInfoBuffer>> {
        self.recording.session_info_buffer()
    }

    /// Copies the immutable variable-header array.
    pub fn variable_headers_buffer(&self) -> Result<Option<VariableHeadersBuffer>> {
        self.recording.variable_headers_buffer()
    }

    /// Sets the next frame to read.
    ///
    /// Seeking to `total_frames` is allowed and positions the cursor at EOF.
    pub fn seek_to_frame(&mut self, frame_index: usize) -> Result<()> {
        if frame_index > self.total_frames() {
            return Err(IRacingSDKError::parse_error(
                "IBT frame seek",
                format!("Frame {frame_index} is outside 0..={}", self.total_frames()),
            ));
        }

        self.next_frame = frame_index;
        Ok(())
    }

    /// Copies and advances past the next frame.
    pub fn read_next_frame(&mut self) -> Result<Option<IbtFrame>> {
        let Some(frame) = self.recording.frame(self.next_frame)? else {
            return Ok(None);
        };

        self.next_frame = self
            .next_frame
            .checked_add(1)
            .ok_or_else(|| IRacingSDKError::parse_error("IBT cursor", "Frame index overflowed"))?;
        Ok(Some(frame))
    }
}

fn validate_metadata_regions<S: RandomAccessSource>(
    source: &S,
    regions: HeaderRegions,
) -> Result<()> {
    for region in [regions.variable_headers(), regions.session_info()]
        .into_iter()
        .flatten()
    {
        if region.offset() < IbtHeader::SIZE {
            return Err(IRacingSDKError::parse_error(
                "IBT layout",
                format!(
                    "Metadata region at {} overlaps the {}-byte IBT header",
                    region.offset(),
                    IbtHeader::SIZE
                ),
            ));
        }
        source.validate_region(region)?;
    }

    Ok(())
}

fn frame_data_start(regions: HeaderRegions) -> usize {
    let metadata_end = [regions.variable_headers(), regions.session_info()]
        .into_iter()
        .flatten()
        .map(ByteRegion::end)
        .max()
        .unwrap_or(IbtHeader::SIZE);

    metadata_end.max(IbtHeader::SIZE)
}

fn validate_frame_layout(
    source_length: usize,
    frame_data_start: usize,
    frame_length: usize,
    advertised_record_count: i32,
) -> Result<usize> {
    let advertised_record_count = usize::try_from(advertised_record_count).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT layout",
            format!("Record count cannot be negative: {advertised_record_count}"),
        )
    })?;
    let remaining = source_length.checked_sub(frame_data_start).ok_or_else(|| {
        IRacingSDKError::parse_error(
            "IBT layout",
            format!(
                "Frame data starts at {frame_data_start}, beyond source length {source_length}"
            ),
        )
    })?;

    if frame_length == 0 {
        if advertised_record_count != 0 {
            return Err(IRacingSDKError::parse_error(
                "IBT layout",
                format!(
                    "Header advertises {advertised_record_count} records with a zero frame length"
                ),
            ));
        }
        if remaining != 0 {
            return Err(IRacingSDKError::parse_error(
                "IBT layout",
                format!("Metadata-only IBT has {remaining} unexplained trailing bytes"),
            ));
        }
        return Ok(0);
    }

    let complete_frames = remaining / frame_length;
    let trailing_bytes = remaining % frame_length;
    if trailing_bytes != 0 {
        return Err(IRacingSDKError::parse_error(
            "IBT layout",
            format!("IBT ends with a partial frame of {trailing_bytes} bytes"),
        ));
    }
    if advertised_record_count != complete_frames {
        return Err(IRacingSDKError::parse_error(
            "IBT layout",
            format!(
                "Disk header advertises {advertised_record_count} records, but the source contains {complete_frames} complete frames"
            ),
        ));
    }

    Ok(complete_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_utils::require_smallest_ibt_fixture, types::WireType};
    use anyhow::Result;

    fn fixture_bytes() -> Result<Vec<u8>> {
        let path = require_smallest_ibt_fixture()?;
        Ok(std::fs::read(path)?)
    }

    #[test]
    fn recording_parses_fixture_regions_and_frames() -> Result<()> {
        let recording = IbtRecording::from_bytes(fixture_bytes()?)?;

        assert_eq!(recording.total_frames(), 12);
        assert_eq!(recording.frame_length(), 48);
        assert!(recording.session_info_buffer()?.is_some());
        assert!(recording.variable_headers_buffer()?.is_some());

        let first = recording.frame(0)?.expect("first frame");
        let bytes: Vec<u8> = first.into_buffer().into();
        assert_eq!(bytes.len(), recording.frame_length());
        assert!(recording.frame(recording.total_frames())?.is_none());
        Ok(())
    }

    #[test]
    fn cursor_derives_position_only_from_frame_index() -> Result<()> {
        let mut reader = IbtReader::from_bytes(fixture_bytes()?)?;

        reader.seek_to_frame(5)?;
        let frame = reader.read_next_frame()?.expect("frame five");
        assert_eq!(frame.index(), 5);
        assert_eq!(reader.current_frame(), 6);

        reader.seek_to_frame(reader.total_frames())?;
        assert!(reader.read_next_frame()?.is_none());
        assert!(reader.seek_to_frame(reader.total_frames() + 1).is_err());
        Ok(())
    }

    #[test]
    fn reader_retains_only_explicit_path_origin() -> Result<()> {
        let path = require_smallest_ibt_fixture()?;
        let from_path = IbtReader::open(&path)?;
        let from_bytes = IbtReader::from_bytes(std::fs::read(&path)?)?;

        assert_eq!(from_path.file_path(), Some(path.as_path()));
        assert_eq!(from_bytes.file_path(), None);
        Ok(())
    }

    #[test]
    fn recording_rejects_partial_final_frame() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        bytes.pop();

        let error = IbtRecording::from_bytes(bytes).expect_err("partial frame must fail");
        assert!(error.to_string().contains("partial frame"));
        Ok(())
    }

    #[test]
    fn recording_rejects_record_count_mismatch() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let record_count_offset = Header::WIRE_SIZE + 28;
        bytes[record_count_offset..record_count_offset + 4].copy_from_slice(&11_i32.to_le_bytes());

        let error = IbtRecording::from_bytes(bytes).expect_err("record mismatch must fail");
        assert!(error.to_string().contains("advertises 11 records"));
        Ok(())
    }
}
