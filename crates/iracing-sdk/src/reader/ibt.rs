//! Concrete immutable reader for recorded iRacing telemetry.
//!
//! Construction materializes the complete source and validates the common and
//! disk headers, metadata regions, frame geometry, and record count. A
//! successfully constructed [`IbtReader`] therefore carries every structural
//! invariant needed by later reads; frame methods perform only index/cursor
//! checks and owned slice copies.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    IRacingSDKError, Result,
    irsdk::{
        DiskSubHeader, FrameBuffer, Header, SessionInfoBuffer, VariableHeader,
        VariableHeadersBuffer, WireType,
    },
    types::IbtHeader,
};

use super::CheckedRegion;

/// One indexed frame copied from an immutable IBT source.
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

/// Parsed, validated, and cursor-positioned access to one immutable IBT source.
#[derive(Debug, Clone)]
pub struct IbtReader {
    bytes: Arc<[u8]>,
    header: IbtHeader,
    session_region: Option<CheckedRegion>,
    variable_headers_region: Option<CheckedRegion>,
    frame_data_start: usize,
    frame_length: usize,
    total_frames: usize,
    next_frame: usize,
    path: Option<PathBuf>,
}

impl IbtReader {
    /// Opens, materializes, parses, and validates an IBT file.
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

        Self::from_materialized(bytes.into(), Some(path))
    }

    /// Parses and validates owned in-memory IBT bytes.
    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Self::from_materialized(bytes.into().into(), None)
    }

    /// Materializes, parses, and validates a sequential IBT source.
    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|source| IRacingSDKError::Buffer {
                context: "materializing IBT source".to_owned(),
                buffer_index: None,
                source: Some(Box::new(source)),
            })?;

        Self::from_materialized(bytes.into(), None)
    }

    fn from_materialized(bytes: Arc<[u8]>, path: Option<PathBuf>) -> Result<Self> {
        let header_bytes = bytes.get(..IbtHeader::SIZE).ok_or_else(|| {
            IRacingSDKError::parse_error(
                "IBT header",
                format!(
                    "Expected at least {} header bytes, found {}",
                    IbtHeader::SIZE,
                    bytes.len()
                ),
            )
        })?;
        let header_buffer: &[u8; IbtHeader::SIZE] = header_bytes.try_into().map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT header",
                format!("Expected exactly {} header bytes", IbtHeader::SIZE),
            )
        })?;
        let header = IbtHeader::try_from_buffer(header_buffer)?;
        header.validate()?;

        let session_region = optional_metadata_region(
            "session info",
            header.header.session_info_offset,
            header.header.session_info_len,
            bytes.len(),
        )?;

        let variable_count = usize::try_from(header.header.variable_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT layout",
                format!(
                    "Variable count cannot be negative: {}",
                    header.header.variable_count
                ),
            )
        })?;
        let variable_headers_length = variable_count
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "IBT layout",
                    "Variable-header region length overflowed",
                )
            })?;
        let variable_headers_region = optional_metadata_region_usize(
            "variable headers",
            header.header.variable_header_offset,
            variable_headers_length,
            bytes.len(),
        )?;

        if session_region
            .zip(variable_headers_region)
            .is_some_and(|(session, variables)| session.overlaps(variables))
        {
            return Err(IRacingSDKError::parse_error(
                "IBT layout",
                "Session-info and variable-header regions overlap",
            ));
        }

        let frame_length = usize::try_from(header.header.buffer_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "IBT layout",
                format!(
                    "Frame length cannot be negative: {}",
                    header.header.buffer_length
                ),
            )
        })?;
        let frame_data_start = [session_region, variable_headers_region]
            .into_iter()
            .flatten()
            .map(CheckedRegion::end)
            .max()
            .unwrap_or(IbtHeader::SIZE)
            .max(IbtHeader::SIZE);
        let total_frames = validate_frame_layout(
            bytes.len(),
            frame_data_start,
            frame_length,
            header.record_count(),
        )?;

        Ok(Self {
            bytes,
            header,
            session_region,
            variable_headers_region,
            frame_data_start,
            frame_length,
            total_frames,
            next_frame: 0,
            path,
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

    /// Returns the filesystem path used by [`Self::open`], when available.
    pub fn file_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the index of the next frame that will be read.
    pub fn current_frame(&self) -> usize {
        self.next_frame
    }

    /// Returns the validated number of complete frames in the recording.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Returns the exact byte length of each telemetry frame.
    pub fn frame_length(&self) -> usize {
        self.frame_length
    }

    /// Returns the absolute byte offset of the first telemetry frame.
    pub fn frame_data_start(&self) -> usize {
        self.frame_data_start
    }

    /// Returns the recording frequency in ticks per second.
    pub fn tick_rate(&self) -> f64 {
        self.header.header.tick_rate as f64
    }

    /// Returns the playback position represented by the cursor.
    pub fn current_time(&self) -> f64 {
        self.next_frame as f64 / self.tick_rate()
    }

    /// Returns the duration represented by all validated frames.
    pub fn duration(&self) -> f64 {
        self.total_frames as f64 / self.tick_rate()
    }

    /// Copies the immutable session-information region.
    pub fn session_info_buffer(&self) -> Result<Option<SessionInfoBuffer>> {
        Ok(self
            .session_region
            .map(|region| SessionInfoBuffer::from_snapshot(self.copy_region(region))))
    }

    /// Copies the immutable variable-header array.
    pub fn variable_headers_buffer(&self) -> Result<Option<VariableHeadersBuffer>> {
        Ok(self
            .variable_headers_region
            .map(|region| VariableHeadersBuffer::from_snapshot(self.copy_region(region))))
    }

    /// Copies a frame by zero-based index without changing the sequential cursor.
    ///
    /// Returns `Ok(None)` when `frame_index` is outside the recording.
    pub fn frame(&self, frame_index: usize) -> Result<Option<IbtFrame>> {
        if frame_index >= self.total_frames {
            return Ok(None);
        }

        // Construction proves that every index below `total_frames` produces a
        // complete in-bounds frame and that these operations cannot overflow.
        let offset = self.frame_data_start + frame_index * self.frame_length;
        let end = offset + self.frame_length;
        let buffer = FrameBuffer::from_snapshot(self.bytes[offset..end].to_vec());

        Ok(Some(IbtFrame {
            index: frame_index,
            buffer,
        }))
    }

    /// Sets the next frame to read.
    ///
    /// Seeking to `total_frames` is allowed and positions the cursor at EOF.
    pub fn seek_to_frame(&mut self, frame_index: usize) -> Result<()> {
        if frame_index > self.total_frames {
            return Err(IRacingSDKError::parse_error(
                "IBT frame seek",
                format!("Frame {frame_index} is outside 0..={}", self.total_frames),
            ));
        }

        self.next_frame = frame_index;
        Ok(())
    }

    /// Copies and advances past the next frame.
    pub fn read_next_frame(&mut self) -> Result<Option<IbtFrame>> {
        let Some(frame) = self.frame(self.next_frame)? else {
            return Ok(None);
        };

        // A successful read proves `next_frame < total_frames`, so incrementing
        // cannot overflow independently of the validated source extent.
        self.next_frame += 1;
        Ok(Some(frame))
    }

    fn copy_region(&self, region: CheckedRegion) -> Vec<u8> {
        self.bytes[region.offset()..region.end()].to_vec()
    }
}

fn optional_metadata_region(
    context: &str,
    offset: i32,
    length: i32,
    source_length: usize,
) -> Result<Option<CheckedRegion>> {
    let length = usize::try_from(length).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT layout",
            format!("{context} length cannot be negative: {length}"),
        )
    })?;
    optional_metadata_region_usize(context, offset, length, source_length)
}

fn optional_metadata_region_usize(
    context: &str,
    offset: i32,
    length: usize,
    source_length: usize,
) -> Result<Option<CheckedRegion>> {
    let offset = usize::try_from(offset).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT layout",
            format!("{context} offset cannot be negative: {offset}"),
        )
    })?;
    if length == 0 {
        return Ok(None);
    }
    if offset < IbtHeader::SIZE {
        return Err(IRacingSDKError::parse_error(
            "IBT layout",
            format!(
                "{context} region at {offset} overlaps the {}-byte IBT header",
                IbtHeader::SIZE
            ),
        ));
    }

    CheckedRegion::new(offset, length, source_length).map(Some)
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
    use crate::test_utils::require_smallest_ibt_fixture;
    use anyhow::Result;

    fn fixture_bytes() -> Result<Vec<u8>> {
        let path = require_smallest_ibt_fixture()?;
        Ok(std::fs::read(path)?)
    }

    #[test]
    fn reader_parses_fixture_regions_and_frames() -> Result<()> {
        let reader = IbtReader::from_bytes(fixture_bytes()?)?;

        assert_eq!(reader.total_frames(), 12);
        assert_eq!(reader.frame_length(), 48);
        assert!(reader.session_info_buffer()?.is_some());
        assert!(reader.variable_headers_buffer()?.is_some());

        let first = reader.frame(0)?.expect("first frame");
        let bytes: Vec<u8> = first.into_buffer().into();
        assert_eq!(bytes.len(), reader.frame_length());
        assert!(reader.frame(reader.total_frames())?.is_none());
        Ok(())
    }

    #[test]
    fn cursor_and_random_access_share_validated_frame_geometry() -> Result<()> {
        let mut reader = IbtReader::from_bytes(fixture_bytes()?)?;

        let random = reader.frame(5)?.expect("frame five");
        let random_bytes: Vec<u8> = random.into_buffer().into();
        assert_eq!(reader.current_frame(), 0);
        reader.seek_to_frame(5)?;
        let sequential = reader.read_next_frame()?.expect("frame five");
        let sequential_bytes: Vec<u8> = sequential.buffer().clone().into();
        assert_eq!(random_bytes, sequential_bytes);
        assert_eq!(sequential.index(), 5);
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
    fn construction_rejects_short_header_and_partial_final_frame() -> Result<()> {
        assert!(IbtReader::from_bytes(vec![0; IbtHeader::SIZE - 1]).is_err());

        let mut bytes = fixture_bytes()?;
        bytes.pop();
        let error = IbtReader::from_bytes(bytes).expect_err("partial frame must fail");
        assert!(error.to_string().contains("partial frame"));
        Ok(())
    }

    #[test]
    fn construction_rejects_record_count_mismatch() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let record_count_offset = Header::WIRE_SIZE + 28;
        bytes[record_count_offset..record_count_offset + 4].copy_from_slice(&11_i32.to_le_bytes());

        let error = IbtReader::from_bytes(bytes).expect_err("record mismatch must fail");
        assert!(error.to_string().contains("advertises 11 records"));
        Ok(())
    }

    #[test]
    fn construction_rejects_overlapping_metadata() -> Result<()> {
        let mut bytes = fixture_bytes()?;
        let variable_offset = i32::from_le_bytes(bytes[28..32].try_into().unwrap());
        bytes[16..20].copy_from_slice(&1_i32.to_le_bytes());
        bytes[20..24].copy_from_slice(&variable_offset.to_le_bytes());

        let error = IbtReader::from_bytes(bytes).expect_err("metadata overlap must fail");
        assert!(error.to_string().contains("overlap"));
        Ok(())
    }
}
