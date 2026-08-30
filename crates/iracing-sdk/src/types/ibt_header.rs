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

    /// Returns the SDK wire-format version from the common header.
    pub fn version(&self) -> i32 {
        self.header.version
    }

    /// Returns the SDK connection-status field recorded in the common header.
    ///
    /// IBT files preserve the field even though their bytes are immutable and
    /// do not represent a currently changing live connection.
    pub fn status(&self) -> StatusField {
        self.header.status
    }

    /// Returns the number of telemetry ticks recorded per second.
    pub fn tick_rate(&self) -> i32 {
        self.header.tick_rate
    }

    /// Returns the session-information update counter stored in the file.
    ///
    /// Unlike live telemetry, an IBT exposes one immutable session-information
    /// region; the value is metadata rather than a signal to poll for changes.
    pub fn session_info_update(&self) -> i32 {
        self.header.session_info_update
    }

    /// Returns the advertised byte length of the session-information region.
    pub fn session_info_length(&self) -> i32 {
        self.header.session_info_len
    }

    /// Returns the absolute file offset of the session-information region.
    pub fn session_info_offset(&self) -> i32 {
        self.header.session_info_offset
    }

    /// Returns the number of variable-header records advertised by the file.
    pub fn variable_count(&self) -> i32 {
        self.header.variable_count
    }

    /// Returns the absolute file offset of the variable-header array.
    pub fn variable_header_offset(&self) -> i32 {
        self.header.variable_header_offset
    }

    /// Returns the common header's telemetry-buffer count.
    ///
    /// Recorded frame layout is determined by the IBT file regions rather than
    /// the rotating live-buffer policy; this accessor preserves the literal
    /// wire value.
    pub fn buffer_count(&self) -> i32 {
        self.header.buffer_count
    }

    /// Returns the byte length of each recorded telemetry frame.
    pub fn buffer_length(&self) -> i32 {
        self.header.buffer_length
    }

    /// Returns the cached current-buffer tick count stored in the common header.
    ///
    /// IBT frame iteration derives its position from the recording cursor and
    /// does not treat this immutable field as a live update signal.
    pub fn current_buffer_tick_count(&self) -> i32 {
        self.header.current_buffer_tick_count
    }

    /// Returns the common header's recorded current-buffer index.
    pub fn current_buffer_index(&self) -> u8 {
        self.header.current_buffer
    }

    /// Returns a copy of every wire-level variable-buffer descriptor.
    ///
    /// The fixed-size array includes unused descriptors. Callers interpreting
    /// live layouts must still honor [`Self::buffer_count`]; IBT readers normally
    /// use the file's sequential frame layout instead.
    pub fn buffers(&self) -> [VariableBuffer; Header::MAX_BUFFERS] {
        self.header.buffers
    }
}

/// `DiskSubHeader` convenience accessors
impl IbtHeader {
    /// Returns the recording start date as the SDK's Unix timestamp value.
    pub fn start_date(&self) -> i64 {
        self.sub_header.start_date
    }

    /// Returns the session time, in seconds, at which recording began.
    pub fn start_time(&self) -> f64 {
        self.sub_header.start_time
    }

    /// Returns the session time, in seconds, at which recording ended.
    pub fn end_time(&self) -> f64 {
        self.sub_header.end_time
    }

    /// Returns the number of laps represented by the recording metadata.
    pub fn lap_count(&self) -> i32 {
        self.sub_header.lap_count
    }

    /// Returns the number of telemetry records advertised by the disk header.
    ///
    /// Readers should validate this metadata against the number of complete
    /// frame-sized regions actually present in the source.
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
