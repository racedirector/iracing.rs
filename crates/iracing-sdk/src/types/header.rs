use std::{
    io::{Read, Seek, SeekFrom},
    ops::Deref,
};

use crate::{IRacingSDKError, Result, parse_utils::bytes_at};

/// iRacing variable buffer information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VariableBuffer {
    /// Tick count when buffer was written
    pub tick_count: i32,
    /// Offset from header to buffer start
    pub buffer_offset: i32,
    /// Tick count begin
    pub tick_count_begin: i32,
    // Padding to maintain alignment
    _pad: [i32; 1],
}

impl VariableBuffer {
    const SIZE: usize = std::mem::size_of::<Self>();

    fn try_from_buffer(buffer: &[u8; Self::SIZE]) -> Result<Self> {
        Ok(Self {
            tick_count: i32::from_le_bytes(*bytes_at(buffer, 0)?),
            buffer_offset: i32::from_le_bytes(*bytes_at(buffer, 4)?),
            tick_count_begin: i32::from_le_bytes(*bytes_at(buffer, 8)?),
            _pad: [i32::from_le_bytes(*bytes_at(buffer, 12)?)],
        })
    }
}

pub const IRSDK_MAX_BUFS: usize = 4;

/// The expected iRacing SDK version
pub const IRSDK_VER: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

fn header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Header validation", details)
}

fn mismatched_version_error(actual: u32) -> IRacingSDKError {
    IRacingSDKError::Version {
        expected: IRSDK_VER as u32,
        found: actual,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// API version
    version: i32,
    /// Status bitfield
    status: i32,
    /// Ticks per second
    tick_rate: i32,
    /// Incremented when session info changes
    session_info_update: i32,
    /// Length in bytes of session info
    session_info_len: i32,
    /// Offset to session info
    session_info_offset: i32,
    /// Number of telemetry variables
    variable_count: i32,
    /// Offset to variable header array
    variable_header_offset: i32,
    /// Number of telemetry buffers
    buffer_count: i32,
    /// Length of each telemetry buffer
    buffer_length: i32,
    /// Cached tick count for the current buffer
    current_buffer_tick_count: i32,
    /// Index of most recently written buffer
    current_buffer: u8,
    /// Alignment padding
    _pad: [u8; 3],
    /// Telemetry buffer descriptors
    buffers: [VariableBuffer; IRSDK_MAX_BUFS],
}

impl Header {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    pub fn try_from_buffer(buffer: &[u8; Self::SIZE]) -> Result<Self> {
        Ok(Self {
            version: i32::from_le_bytes(*bytes_at(buffer, 0)?),
            status: i32::from_le_bytes(*bytes_at(buffer, 4)?),
            tick_rate: i32::from_le_bytes(*bytes_at(buffer, 8)?),
            session_info_update: i32::from_le_bytes(*bytes_at(buffer, 12)?),
            session_info_len: i32::from_le_bytes(*bytes_at(buffer, 16)?),
            session_info_offset: i32::from_le_bytes(*bytes_at(buffer, 20)?),
            variable_count: i32::from_le_bytes(*bytes_at(buffer, 24)?),
            variable_header_offset: i32::from_le_bytes(*bytes_at(buffer, 28)?),
            buffer_count: i32::from_le_bytes(*bytes_at(buffer, 32)?),
            buffer_length: i32::from_le_bytes(*bytes_at(buffer, 36)?),
            current_buffer_tick_count: i32::from_le_bytes(*bytes_at(buffer, 40)?),
            current_buffer: bytes_at::<1>(buffer, 44)?[0],
            _pad: *bytes_at(buffer, 45)?,
            // buffers: std::array::try_from_fn(|index| {
            //     VariableBuffer::try_from_buffer(buffer, 48 + index * VariableBuffer::SIZE)
            // })?,
            buffers: [
                VariableBuffer::try_from_buffer(bytes_at(buffer, 48)?)?,
                VariableBuffer::try_from_buffer(bytes_at(buffer, 64)?)?,
                VariableBuffer::try_from_buffer(bytes_at(buffer, 80)?)?,
                VariableBuffer::try_from_buffer(bytes_at(buffer, 96)?)?,
            ],
        })
    }

    fn validate(&self) -> Result<()> {
        // Check that core fields are not equal to 0
        if self.version == 0
            && self.status == 0
            && self.tick_rate == 0
            && self.variable_count == 0
            && self.buffer_length == 0
        {
            return Err(header_validation_error("Header appears to be all zeros"));
        }

        // Check SDK version
        if self.version != IRSDK_VER {
            return Err(mismatched_version_error(self.version as u32));
        }

        // Validate offset fields are non-negative (defensive correctness)
        if self.session_info_offset < 0 {
            return Err(header_validation_error(
                "Session info offset cannot be negative",
            ));
        }

        if self.session_info_len < 0 {
            return Err(header_validation_error(
                "Session info length cannot be negative",
            ));
        }

        self.validate_session_offset()?;

        if self.variable_header_offset < 0 {
            return Err(header_validation_error(
                "Variable header offset cannot be negative",
            ));
        }

        self.validate_variable_offset()?;

        if self.tick_rate < 0 || self.session_info_len < -1 {
            return Err(header_validation_error(
                "Header contains invalid negative values",
            ));
        }

        Ok(())
    }

    fn validate_session_offset(&self) -> Result<()> {
        if self.session_info_offset > 0
            && self.session_info_len > 0
            && self
                .session_info_offset
                .saturating_add(self.session_info_len)
                < self.session_info_offset
        {
            return Err(header_validation_error(
                "Session info offset + length causes overflow",
            ));
        }

        Ok(())
    }

    fn validate_variable_offset(&self) -> Result<()> {
        if self.variable_header_offset > 0 && self.variable_count > 0 {}

        Ok(())
    }
}

pub struct Unvalidated;
pub struct Ibt {
    sub_header: DiskSubHeader,
}
pub struct Live;

pub struct IRSDKHeader<State = Unvalidated> {
    raw: Header,
    state: State,
}

impl<State> Deref for IRSDKHeader<State> {
    type Target = Header;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl IRSDKHeader<Unvalidated> {
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        tracing::trace!("Reading header: ({} bytes)", Header::SIZE);

        let mut buffer = [0u8; Header::SIZE];
        reader.read_exact(&mut buffer).map_err(|e| {
            IRacingSDKError::parse_error(
                "Header reading",
                format!("Failed to read {} header bytes: {}", Header::SIZE, e),
            )
        })?;

        Self::try_from_buffer(&buffer)
    }

    pub fn try_from_buffer(buffer: &[u8; Header::SIZE]) -> Result<Self> {
        let header = Header::try_from_buffer(buffer)?;

        Ok(Self {
            raw: header,
            state: Unvalidated,
        })
    }
}

pub type RawHeader = IRSDKHeader<Unvalidated>;
pub type IbtHeader = IRSDKHeader<Ibt>;
pub type LiveHeader = IRSDKHeader<Live>;

impl RawHeader {
    pub fn validate_live(&self) -> Result<LiveHeader> {
        self.raw.validate()?;

        if self.buffer_count < 3 || self.buffer_count > 4 {
            return Err(header_validation_error(format!(
                "Expected 3-4 buffers, found {}",
                self.buffer_count
            )));
        }

        todo!()
    }

    pub fn validate_ibt(&self) -> Result<IbtHeader> {
        self.raw.validate()?;

        // TODO: Verify if the buffer count should be 0 in IBT files

        todo!()
    }
}

impl IbtHeader {
    pub fn sub_header(&self) -> &DiskSubHeader {
        &self.state.sub_header
    }

    pub fn total_frames(&self) -> bool {
        todo!();
    }
}

impl LiveHeader {
    pub fn is_connected(&self) -> bool {
        self.status & IRSDK_STATUS_CONNECTED != 0
    }

    pub fn session_info_changed(&self, last_update: i32) -> bool {
        self.session_info_update != last_update
    }
}

/// IBT disk sub-header (IBT-specific structure, `irsdk_diskSubHeader`).
///
/// Stored just before the variable header array (at
/// `header.var_header_offset - IRSDK_DISK_SUBHEADER_SIZE`) and provides timing and record-count
/// metadata specific to `.ibt` replay files.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DiskSubHeader {
    /// Unix timestamp (`time_t`) of the session start date.
    start_date: i64,
    /// Session start time in seconds since session midnight.
    start_time: f64,
    /// Session end time in seconds since session midnight.
    end_time: f64,
    /// Number of laps completed during the recorded session.
    lap_count: i32,
    /// Total number of telemetry frames (records) in the file.
    record_count: i32,
}

impl DiskSubHeader {
    const SIZE: usize = std::mem::size_of::<Self>();

    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buffer = [0u8; Self::SIZE];
        reader.read_exact(&mut buffer).map_err(|e| {
            IRacingSDKError::parse_error(
                "DiskSubHeader",
                format!("Failed to read {} disk sub-header bytes: {}", Self::SIZE, e),
            )
        })?;

        Self::try_from_buffer(&buffer)
    }

    pub fn try_from_reader_offset<R: Read + Seek>(reader: &mut R, offset: usize) -> Result<Self> {
        reader.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
            IRacingSDKError::parse_error(
                "DiskSubHeader",
                format!(
                    "Failed to seek to disk sub-header at offset {}: {}",
                    offset, e
                ),
            )
        })?;

        Self::try_from_reader(reader)
    }

    pub fn try_from_buffer(buffer: &[u8; Self::SIZE]) -> Result<Self> {
        Ok(Self {
            start_date: i64::from_le_bytes(*bytes_at(buffer, 0)?),
            start_time: f64::from_le_bytes(*bytes_at(buffer, 8)?),
            end_time: f64::from_le_bytes(*bytes_at(buffer, 16)?),
            lap_count: i32::from_le_bytes(*bytes_at(buffer, 24)?),
            record_count: i32::from_le_bytes(*bytes_at(buffer, 28)?),
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::mem;

    #[test]
    fn variable_buffer_size_matches_expected_layout() {
        assert_eq!(mem::size_of::<VariableBuffer>(), 16); // 4 * i32
        assert_eq!(std::mem::align_of::<VariableBuffer>(), 4);
    }

    #[test]
    fn header_size_matches_expected_layout() {
        // Ensure struct packing matches C layout
        assert_eq!(mem::size_of::<Header>(), Header::SIZE);
        assert_eq!(mem::align_of::<Header>(), 4);
    }

    #[test]
    fn disk_sub_header_size_matches_expected_layout() {
        // Ensure struct packing matches C layout
        assert_eq!(mem::size_of::<DiskSubHeader>(), DiskSubHeader::SIZE);
        assert_eq!(mem::align_of::<DiskSubHeader>(), 8);
    }

    #[test]
    fn irsdk_header_size_matches_header_layout() {
        // RawHeader should be the same size of the header (112), alignment 4
        assert_eq!(mem::size_of::<RawHeader>(), 112);
        assert_eq!(mem::align_of::<RawHeader>(), 4);

        // IbtHeader should be the size of the header + disk sub header (144), alignment 8, due to the sub header
        assert_eq!(mem::size_of::<IbtHeader>(), 144);
        assert_eq!(mem::align_of::<IbtHeader>(), 8);

        // LiveHeader should be the size of the header (112), alignment 4
        assert_eq!(mem::size_of::<LiveHeader>(), 112);
        assert_eq!(mem::align_of::<LiveHeader>(), 4);
    }
}
