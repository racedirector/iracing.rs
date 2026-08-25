//! Headers associated with the iRacing SDK

use std::io::Read;
use type_layout::TypeLayout;

use crate::{IRacingSDKError, Result, parse_utils::bytes_at};

use super::{DiskSubHeader, Header, VariableBuffer, WireType, irsdk::StatusField};

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
    pub const SIZE: usize = Header::WIRE_SIZE + DiskSubHeader::WIRE_SIZE;

    /// Attempts to read and prase the header and sub header from the provided reader.
    /// It's assumed that the sub header immediately follows the header in the buffer.
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buffer = [0u8; Self::SIZE];

        reader.read_exact(&mut buffer).map_err(|e| {
            IRacingSDKError::parse_error(
                "IbtHeader",
                format!("Failed to read {} header bytes: {}", Self::SIZE, e),
            )
        })?;

        Self::try_from_buffer(&buffer)
    }

    /// Attempts to parse an `IbtHeader` from the provided buffer.
    pub fn try_from_buffer(buffer: &[u8; IbtHeader::SIZE]) -> Result<Self> {
        // Load the first `Header::SIZE` bytes into a buffer
        let header_buffer: &[u8; Header::WIRE_SIZE] = bytes_at(buffer, 0)?;

        // Load the remaining bytes from the end of the header into a buffer
        let sub_header_buffer: &[u8; DiskSubHeader::WIRE_SIZE] =
            bytes_at(buffer, Header::WIRE_SIZE)?;

        Ok(Self {
            // Try to parse a header
            header: Header::read_from_bytes(header_buffer)?,
            // Try to parse a sub header
            sub_header: DiskSubHeader::read_from_bytes(sub_header_buffer)?,
        })
    }
}

// `Header` convenience accessors
impl IbtHeader {
    /// Validates the header
    pub fn validate(&self) -> Result<()> {
        self.header.validate_ibt()
    }

    /// Indicates if the header is a valid IBT header.
    pub fn is_valid(&self) -> bool {
        self.header.validate_ibt().is_ok()
    }

    pub fn version(&self) -> i32 {
        self.header.version
    }

    pub fn status(&self) -> StatusField {
        self.header.status
    }

    pub fn tick_rate(&self) -> i32 {
        self.header.tick_rate
    }

    pub fn session_info_update(&self) -> i32 {
        self.header.session_info_update
    }

    pub fn session_info_length(&self) -> i32 {
        self.header.session_info_len
    }

    pub fn session_info_offset(&self) -> i32 {
        self.header.session_info_offset
    }

    pub fn variable_count(&self) -> i32 {
        self.header.variable_count
    }

    pub fn variable_header_offset(&self) -> i32 {
        self.header.variable_header_offset
    }

    pub fn buffer_count(&self) -> i32 {
        self.header.buffer_count
    }

    pub fn buffer_length(&self) -> i32 {
        self.header.buffer_length
    }

    pub fn current_buffer_tick_count(&self) -> i32 {
        self.header.current_buffer_tick_count
    }

    pub fn current_buffer_index(&self) -> u8 {
        self.header.current_buffer
    }

    pub fn buffers(&self) -> [VariableBuffer; Header::MAX_BUFFERS] {
        self.header.buffers
    }
}

/// `DiskSubHeader` convenience accessors
impl IbtHeader {
    pub fn start_date(&self) -> i64 {
        self.sub_header.start_date
    }

    pub fn start_time(&self) -> f64 {
        self.sub_header.start_time
    }

    pub fn end_time(&self) -> f64 {
        self.sub_header.end_time
    }

    pub fn lap_count(&self) -> i32 {
        self.sub_header.lap_count
    }

    pub fn record_count(&self) -> i32 {
        self.sub_header.record_count
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::mem;

    #[test]
    fn ibt_header_size_matches_header_layout() {
        // IbtHeader should be the size of the header + disk sub header (144), alignment 8, due to the sub header
        assert_eq!(IbtHeader::SIZE, 144);
        assert_eq!(mem::align_of::<IbtHeader>(), 8);
    }
}
