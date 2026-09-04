use std::io::Read;
use type_layout::TypeLayout;

use super::{
    constants::{IRSDK_MAX_BUFS as IRSDK_MAX_BUFFERS, IRSDK_VER as IRSDK_VERSION},
    error::{header_validation_error, mismatched_version_error},
    flags::StatusField,
    variable_buffer::VariableBuffer,
    variable_header::VariableHeader,
};
use crate::{IRacingSDKError, Result, types::irsdk::wire_type::WireType};

/// An iRacing SDK header.
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct Header {
    /// API version
    pub version: i32,
    /// Status bitfield
    pub status: StatusField,
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
    /// Cached tick count for the current buffer (`irsdk_header::curBufTickCount`)
    pub current_buffer_tick_count: i32,
    /// Index of most recently written buffer (`irsdk_header::curBuf`)
    pub current_buffer: u8,
    /// Alignment padding (`irsdk_header::pad1`)
    _pad: [u8; 3],
    /// Telemetry buffer descriptors
    pub buffers: [VariableBuffer; Self::MAX_BUFFERS],
}

/// Constructors
impl Header {
    /// The max number of buffers that can be found in the buffers array.
    pub const MAX_BUFFERS: usize = IRSDK_MAX_BUFFERS;
    const MAX_LIVE_VARIABLES: i32 = 5_000;
    const MAX_LIVE_BUFFER_LENGTH: i32 = 10_000_000;

    /// Reads a buffer of `Self::WIRE_SIZE` from the provided reader and uses
    /// the `read_from_bytes` of `WireType` to create an instance of `Self`
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buffer = [0u8; Self::WIRE_SIZE];

        reader.read_exact(&mut buffer).map_err(|e| {
            IRacingSDKError::parse_error(
                "Header reading",
                format!("Failed to read {} header bytes: {}", Header::WIRE_SIZE, e),
            )
        })?;

        Self::read_from_bytes(&buffer)
    }

    #[allow(clippy::too_many_arguments)]
    /// Constructs a header value, filling the ABI padding automatically.
    pub fn new(
        version: i32,
        status: StatusField,
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
}

/// Validation utilities
impl Header {
    /// Performs general validation on the header for common corruption indicators and
    /// invalid values.
    pub fn validate(&self) -> Result<()> {
        // Check that core fields are not equal to 0
        if self.version == 0
            && self.status.bits() == 0
            && self.tick_rate == 0
            && self.variable_count == 0
            && self.buffer_length == 0
        {
            return Err(header_validation_error("Header appears to be all zeros"));
        }

        // Check SDK version
        if self.version != IRSDK_VERSION {
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
                .checked_mul(VariableHeader::WIRE_SIZE as i32)
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

        if self.variable_count > Self::MAX_LIVE_VARIABLES {
            return Err(header_validation_error(format!(
                "Number of variables exceeds live limit of {}",
                Self::MAX_LIVE_VARIABLES
            )));
        }

        if self.buffer_count < 3 || self.buffer_count > 4 {
            return Err(header_validation_error(format!(
                "Expected 3-4 buffers, found {}",
                self.buffer_count
            )));
        }

        if self.buffer_length <= 0 || self.buffer_length > Self::MAX_LIVE_BUFFER_LENGTH {
            return Err(header_validation_error(format!(
                "Expected buffer length in 1..={}, found {}",
                Self::MAX_LIVE_BUFFER_LENGTH,
                self.buffer_length,
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
        self.status.contains(StatusField::CONNECTED)
    }

    /// Indicates whether the session info has changed compared to `last_update`.
    pub fn session_info_changed(&self, last_update: i32) -> bool {
        self.session_info_update != last_update
    }
}

unsafe impl WireType for Header {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    fn valid_live_header() -> Header {
        Header::new(
            IRSDK_VERSION,
            StatusField::CONNECTED,
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
        header.variable_count = Header::MAX_LIVE_VARIABLES + 1;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffer_length = 0;
        assert!(header.validate_live().is_err());

        let mut header = valid_live_header();
        header.buffer_length = Header::MAX_LIVE_BUFFER_LENGTH + 1;
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
    fn header_layout_matches_iracing_abi() {
        assert_eq!(Header::WIRE_SIZE, 112);

        assert_eq!(align_of::<Header>(), 4);

        assert_eq!(offset_of!(Header, version), 0);
        assert_eq!(offset_of!(Header, status), 4);
        assert_eq!(offset_of!(Header, tick_rate), 8);
        assert_eq!(offset_of!(Header, session_info_update), 12);
        assert_eq!(offset_of!(Header, session_info_len), 16);
        assert_eq!(offset_of!(Header, session_info_offset), 20);
        assert_eq!(offset_of!(Header, variable_count), 24);
        assert_eq!(offset_of!(Header, variable_header_offset), 28);
        assert_eq!(offset_of!(Header, buffer_count), 32);
        assert_eq!(offset_of!(Header, buffer_length), 36);
        assert_eq!(offset_of!(Header, current_buffer_tick_count), 40);
        assert_eq!(offset_of!(Header, current_buffer), 44);
        assert_eq!(offset_of!(Header, _pad), 45);
        assert_eq!(offset_of!(Header, buffers), 48);
    }
}
