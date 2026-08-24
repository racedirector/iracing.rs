//! Headers associated with the iRacing SDK

use std::io::Read;
use type_layout::TypeLayout;

use crate::{
    IRacingSDKError, Result, VariableInfo,
    parse_utils::{self, bytes_at},
};

/// The expected iRacing SDK version
pub const IRSDK_VER: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

const IRSDK_MAX_STRING: usize = 32;
const IRSDK_MAX_DESC: usize = 64;
const MAX_LIVE_VARIABLES: i32 = 5_000;
const MAX_LIVE_BUFFER_LENGTH: i32 = 10_000_000;

/// Returns whether the connected status bit is set.
pub fn status_is_connected(status: i32) -> bool {
    status & IRSDK_STATUS_CONNECTED != 0
}

fn header_validation_error(details: impl Into<String>) -> IRacingSDKError {
    IRacingSDKError::parse_error("Header validation", details)
}

fn mismatched_version_error(actual: u32) -> IRacingSDKError {
    IRacingSDKError::Version {
        expected: IRSDK_VER as u32,
        found: actual,
    }
}

/// iRacing variable buffer information
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct VariableBuffer {
    /// Tick count when buffer was written
    pub tick_count: i32,
    /// Offset from header to buffer start
    pub buffer_offset: i32,
    /// Tick count begin
    pub tick_count_begin: i32,
    /// Padding to maintain alignment
    _pad: [i32; 1],
}

impl VariableBuffer {
    /// Size of one variable-buffer descriptor in the SDK wire format.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Reads a descriptor from its fixed-size wire representation.
    #[inline]
    pub fn read_from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast()) }
    }

    /// Convenience constructor. Automatically inserts padding.
    pub fn new(tick_count: i32, buffer_offset: i32, tick_count_begin: i32) -> Self {
        Self {
            tick_count,
            buffer_offset,
            tick_count_begin,
            _pad: [0; 1],
        }
    }

    /// Attempts to parse a VariableBuffer from the provided buffer.
    pub fn try_from_buffer(buffer: &[u8; Self::SIZE]) -> Result<Self> {
        Ok(Self {
            tick_count: i32::from_le_bytes(*bytes_at(buffer, 0)?),
            buffer_offset: i32::from_le_bytes(*bytes_at(buffer, 4)?),
            tick_count_begin: i32::from_le_bytes(*bytes_at(buffer, 8)?),
            _pad: [i32::from_le_bytes(*bytes_at(buffer, 12)?)],
        })
    }
}

/// An iRacing SDK header.
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct Header {
    /// API version
    pub version: i32,
    /// Status bitfield
    pub status: i32,
    /// Ticks per second
    pub tick_rate: i32,
    /// Incremented when session info changes
    pub session_info_update: i32,
    /// Length in bytes of session info
    pub session_info_len: i32,
    /// Offset to session info
    pub session_info_offset: i32,
    /// Number of telemetry variables
    pub variable_count: i32,
    /// Offset to variable header array
    pub variable_header_offset: i32,
    /// Number of telemetry buffers
    pub buffer_count: i32,
    /// Length of each telemetry buffer
    pub buffer_length: i32,
    /// Cached tick count for the current buffer
    pub current_buffer_tick_count: i32,
    /// Index of most recently written buffer
    pub current_buffer: u8,
    // /// Alignment padding
    _pad: [u8; 3],
    /// Telemetry buffer descriptors
    pub buffers: [VariableBuffer; Self::MAX_BUFFERS],
}

impl Header {
    /// The size of the header, in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
    /// The max number of buffers that can be written to the header
    pub const MAX_BUFFERS: usize = 4;

    /// Reads a header snapshot from its fixed-size SDK wire representation.
    #[inline]
    pub fn read_from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast()) }
    }

    #[allow(clippy::too_many_arguments)]
    /// Constructs a header value, filling the ABI padding automatically.
    pub fn new(
        version: i32,
        status: i32,
        tick_rate: i32,
        session_info_update: i32,
        session_info_len: i32,
        session_info_offset: i32,
        variable_count: i32,
        variable_header_offset: i32,
        buffer_count: i32,
        buffer_length: i32,
        current_buffer_tick_count: i32,
        current_buffer: u8,
        buffers: [VariableBuffer; Self::MAX_BUFFERS],
    ) -> Self {
        Self {
            version,
            status,
            tick_rate,
            session_info_update,
            session_info_len,
            session_info_offset,
            variable_count,
            variable_header_offset,
            buffer_count,
            buffer_length,
            current_buffer_tick_count,
            current_buffer,
            _pad: [0; 3],
            buffers,
        }
    }

    /// Attempts to read and parse a header from the provided reader.
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

    /// Attempts to parse a header from the provided buffer, and performs basic validation.
    /// Consumers are expected to use `validate_live` and `validate_ibt` for their use-case.
    pub fn try_from_buffer(buffer: &[u8; Self::SIZE]) -> Result<Self> {
        let header = Self {
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
        };

        header.validate()?;

        Ok(header)
    }

    /// Performs general validation on the header for common corruption indicators and
    /// invalid values.
    pub fn validate(&self) -> Result<()> {
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

        // Sanity check for negative values

        // Check for negative variable count
        if self.variable_count < 0 {
            return Err(header_validation_error(
                "Number of variables cannot be negative",
            ));
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

        if self.tick_rate < 0 {
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
                .checked_add(self.session_info_len)
                .is_none()
        {
            return Err(header_validation_error(
                "Session info offset + length causes overflow",
            ));
        }

        Ok(())
    }

    fn validate_variable_offset(&self) -> Result<()> {
        if self.variable_header_offset > 0 && self.variable_count > 0 {
            let variable_bytes = self
                .variable_count
                .checked_mul(VariableHeader::SIZE as i32)
                .ok_or_else(|| header_validation_error("Variable header array size overflows"))?;

            self.variable_header_offset
                .checked_add(variable_bytes)
                .ok_or_else(|| {
                    header_validation_error("Variable header offset + length causes overflow")
                })?;
        }

        Ok(())
    }

    /// Performs validation on the header for common live corruption indicators and invalid
    /// values.
    pub fn validate_live(&self) -> Result<()> {
        self.validate()?;

        if !(1..=1_000).contains(&self.tick_rate) {
            return Err(header_validation_error(format!(
                "Expected tick rate in 1..=1000, found {}",
                self.tick_rate
            )));
        }

        if self.variable_count > MAX_LIVE_VARIABLES {
            return Err(header_validation_error(format!(
                "Number of variables exceeds live limit of {MAX_LIVE_VARIABLES}"
            )));
        }

        if self.buffer_count < 3 || self.buffer_count > 4 {
            return Err(header_validation_error(format!(
                "Expected 3-4 buffers, found {}",
                self.buffer_count
            )));
        }

        if self.buffer_length <= 0 || self.buffer_length > MAX_LIVE_BUFFER_LENGTH {
            return Err(header_validation_error(format!(
                "Expected buffer length in 1..={MAX_LIVE_BUFFER_LENGTH}, found {}",
                self.buffer_length
            )));
        }

        if usize::from(self.current_buffer) >= self.buffer_count as usize {
            return Err(header_validation_error(format!(
                "Current buffer index {} is outside buffer count {}",
                self.current_buffer, self.buffer_count
            )));
        }

        for (index, buffer) in self.buffers[..self.buffer_count as usize]
            .iter()
            .enumerate()
        {
            if buffer.buffer_offset < 0 {
                return Err(header_validation_error(format!(
                    "Buffer {index} offset cannot be negative"
                )));
            }

            buffer
                .buffer_offset
                .checked_add(self.buffer_length)
                .ok_or_else(|| {
                    header_validation_error(format!(
                        "Buffer {index} offset + length causes overflow"
                    ))
                })?;
        }

        Ok(())
    }

    /// Performs validation on the header for common disk corruption indicators and invalid
    /// values.
    pub fn validate_ibt(&self) -> Result<()> {
        self.validate()?;

        if self.buffer_count < 0 {
            return Err(header_validation_error("Buffer length cannot be negative"));
        }

        // !!!: These may be relevant in the common validation
        if self.buffer_length > 100_000_000 {
            return Err(header_validation_error(
                "Buffer length is unreasonably large",
            ));
        }

        if self.variable_count > 10_000 {
            return Err(header_validation_error(
                "Number of variables is unreasonably large",
            ));
        }

        Ok(())
    }

    /// Convenience for indicating if the header is generall considered valid.
    /// This value is not cached and performs validation.
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Indicates whether the header is connected.
    pub fn is_connected(&self) -> bool {
        self.status & IRSDK_STATUS_CONNECTED != 0
    }

    /// Indicates whether the session info has changed compared to `last_update`.
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
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct DiskSubHeader {
    /// Unix timestamp (`time_t`) of the session start date.
    pub start_date: i64,
    /// Session start time in seconds since session midnight.
    pub start_time: f64,
    /// Session end time in seconds since session midnight.
    pub end_time: f64,
    /// Number of laps completed during the recorded session.
    pub lap_count: i32,
    /// Total number of telemetry frames (records) in the file.
    pub record_count: i32,
}

impl DiskSubHeader {
    /// The size of the sub header in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Attempts to read and parse a sub header from the provided reader.
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

    /// Attempts to parse a sub header from the provided buffer.
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

/// The header and sub header in an IBT file.
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct IbtHeader {
    /// The header
    pub header: Header,
    /// The sub header
    pub sub_header: DiskSubHeader,
}

impl IbtHeader {
    /// The size of the header in bytes
    pub const SIZE: usize = Header::SIZE + DiskSubHeader::SIZE;

    /// Attempts to read and prase the header and sub header from the provided reader.
    /// It's assumed that the sub header immediately follows the header in the buffer.
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buffer = [0u8; Self::SIZE];

        reader.read_exact(&mut buffer).map_err(|e| {
            IRacingSDKError::parse_error(
                "IbtHeader",
                format!("Failed to read {} header bytes: {}", Header::SIZE, e),
            )
        })?;

        Self::try_from_buffer(&buffer)
    }

    /// Attempts to parse an `IbtHeader` from the provided buffer.
    pub fn try_from_buffer(buffer: &[u8; IbtHeader::SIZE]) -> Result<Self> {
        // Load the first `Header::SIZE` bytes into a buffer
        let header_buffer: &[u8; Header::SIZE] = bytes_at(buffer, 0)?;

        // Load the remaining bytes from the end of the header into a buffer
        let sub_header_buffer: &[u8; DiskSubHeader::SIZE] = bytes_at(buffer, Header::SIZE)?;

        Ok(Self {
            // Try to parse a header
            header: Header::try_from_buffer(header_buffer)?,
            // Try to parse a sub header
            sub_header: DiskSubHeader::try_from_buffer(sub_header_buffer)?,
        })
    }

    /// Validates the header
    pub fn validate(&self) -> Result<()> {
        self.header.validate_ibt()
    }

    /// Indicates if the header is a valid IBT header.
    pub fn is_valid(&self) -> bool {
        self.header.validate_ibt().is_ok()
    }
}

/// iRacing variable header structure matching the C SDK layout
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct VariableHeader {
    /// Variable type (irsdk_VarType enum)
    variable_type: i32,
    /// Offset in bytes from buffer start
    offset: i32,
    /// Number of elements (1 for scalar, >1 for arrays)
    count: i32,
    /// Whether the count field should be interpreted as time
    count_as_time: u8,
    /// Padding for alignment (matches 3-byte C padding)
    _pad: [u8; 3],
    /// Variable name (32 bytes, null-terminated)
    name: [u8; IRSDK_MAX_STRING],
    /// Variable description (64 bytes, null-terminated)
    description: [u8; IRSDK_MAX_DESC],
    /// Variable units (32 bytes, null-terminated)
    unit: [u8; IRSDK_MAX_STRING],
}

impl VariableHeader {
    /// Size of one variable header in the SDK wire format.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Reads a variable header from its SDK wire representation.
    #[inline]
    pub fn read_from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast()) }
    }

    /// Creates a sized slice of bytes and internally uses `read_from_bytes`.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes: &[u8; Self::SIZE] = bytes_at(bytes, 0)?;
        Ok(Self::read_from_bytes(bytes))
    }
}

impl TryFrom<VariableHeader> for VariableInfo {
    type Error = IRacingSDKError;

    fn try_from(value: VariableHeader) -> Result<Self> {
        Ok(VariableInfo {
            name: parse_utils::c_string_to_string(&value.name),
            description: parse_utils::c_string_to_string(&value.description),
            units: parse_utils::c_string_to_string(&value.unit),
            data_type: value.variable_type.try_into()?,
            offset: usize::try_from(value.offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "TryFrom<VariableHeader> for VariableInfo",
                    format!("Could not convert {} to usize", value.offset),
                )
            })?,
            count: usize::try_from(value.count).map_err(|_| {
                IRacingSDKError::parse_error(
                    "TryFrom<VariableHeader> for VariableInfo",
                    format!("Could not convert {} to usize", value.count),
                )
            })?,
            count_as_time: value.count_as_time != 0,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::mem;

    fn valid_live_header() -> Header {
        Header::new(
            IRSDK_VER,
            IRSDK_STATUS_CONNECTED,
            60,
            0,
            1_000,
            112,
            100,
            1_112,
            4,
            8_000,
            10,
            0,
            [
                VariableBuffer::new(10, 20_000, 10),
                VariableBuffer::new(9, 28_000, 9),
                VariableBuffer::new(8, 36_000, 8),
                VariableBuffer::new(7, 44_000, 7),
            ],
        )
    }

    #[test]
    fn live_header_validation_accepts_valid_layout() {
        valid_live_header().validate_live().unwrap();
    }

    #[test]
    fn live_header_validation_rejects_invalid_scalar_fields() {
        let mut header = valid_live_header();
        header.tick_rate = 0;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.tick_rate = 1_001;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.variable_count = MAX_LIVE_VARIABLES + 1;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffer_length = 0;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffer_length = MAX_LIVE_BUFFER_LENGTH + 1;
        assert!(header.validate_live().is_err());
    }

    #[test]
    fn live_header_validation_rejects_invalid_buffer_layout() {
        let mut header = valid_live_header();
        header.buffer_count = 2;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.current_buffer = 4;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffers[1] = VariableBuffer::new(9, -1, 9);
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffers[1] = VariableBuffer::new(9, i32::MAX, 9);
        assert!(header.validate_live().is_err());
    }

    #[test]
    fn common_header_validation_rejects_offset_overflow() {
        let mut header = valid_live_header();
        header.session_info_offset = i32::MAX;
        assert!(header.validate().is_err());

        let mut header = valid_live_header();
        header.variable_header_offset = i32::MAX;
        assert!(header.validate().is_err());
    }

    #[test]
    fn variable_buffer_layout_matches_iracing_abi() {
        assert_eq!(VariableBuffer::SIZE, 16);
        assert_eq!(mem::align_of::<VariableBuffer>(), 4);

        assert_eq!(mem::offset_of!(VariableBuffer, tick_count), 0);
        assert_eq!(mem::offset_of!(VariableBuffer, buffer_offset), 4);
        assert_eq!(mem::offset_of!(VariableBuffer, tick_count_begin), 8);
    }

    #[test]
    fn header_layout_matches_iracing_abi() {
        assert_eq!(Header::SIZE, 48 + 16 * Header::MAX_BUFFERS);
        assert_eq!(mem::align_of::<Header>(), 4);

        assert_eq!(mem::offset_of!(Header, current_buffer), 44);
        assert_eq!(mem::offset_of!(Header, buffers), 48);
    }

    #[test]
    fn disk_sub_header_layout_matches_iracing_abi() {
        assert_eq!(DiskSubHeader::SIZE, 32);
        assert_eq!(mem::align_of::<DiskSubHeader>(), 8);

        assert_eq!(mem::offset_of!(DiskSubHeader, start_date), 0);
        assert_eq!(mem::offset_of!(DiskSubHeader, start_time), 8);
        assert_eq!(mem::offset_of!(DiskSubHeader, end_time), 16);
        assert_eq!(mem::offset_of!(DiskSubHeader, lap_count), 24);
        assert_eq!(mem::offset_of!(DiskSubHeader, record_count), 28);
    }

    #[test]
    fn variable_header_layout_matches_iracing_abi() {
        assert_eq!(VariableHeader::SIZE, 144);
        assert_eq!(mem::align_of::<VariableHeader>(), 4);

        assert_eq!(mem::offset_of!(VariableHeader, variable_type), 0);
        assert_eq!(mem::offset_of!(VariableHeader, offset), 4);
        assert_eq!(mem::offset_of!(VariableHeader, count), 8);
        assert_eq!(mem::offset_of!(VariableHeader, count_as_time), 12);
        assert_eq!(mem::offset_of!(VariableHeader, _pad), 13);
        assert_eq!(mem::offset_of!(VariableHeader, name), 16);
        assert_eq!(mem::offset_of!(VariableHeader, description), 48);
        assert_eq!(mem::offset_of!(VariableHeader, unit), 112);
    }

    #[test]
    fn ibt_header_size_matches_header_layout() {
        // IbtHeader should be the size of the header + disk sub header (144), alignment 8, due to the sub header
        assert_eq!(IbtHeader::SIZE, 144);
        assert_eq!(mem::align_of::<IbtHeader>(), 8);
    }
}
