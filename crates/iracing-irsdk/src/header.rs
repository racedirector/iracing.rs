use std::io::Read;
use type_layout::TypeLayout;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use super::{StatusField, VariableBuffer, constants::IRSDK_MAX_BUFS as IRSDK_MAX_BUFFERS};
use crate::{Result, parse_utils::read_wire_bytes};

/// An iRacing SDK header.
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout, FromBytes, IntoBytes, KnownLayout, Immutable)]
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
    pub session_info_length: i32,
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

    /// Decodes one complete SDK header from its exact wire representation.
    ///
    /// This does not validate the decoded field values.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::WireSize`] when `bytes` is not exactly the size
    /// of an SDK header.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        read_wire_bytes(bytes)
    }

    /// Reads and decodes one complete SDK header from `reader`.
    ///
    /// # Errors
    ///
    /// Returns a parse error if `reader` cannot supply a complete header.
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_from_io(reader).map_err(|error| {
            crate::Error::parse(
                "Header::try_from_reader",
                format!("Failed to read disk header: {error}"),
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    /// Constructs a header value, filling the ABI padding automatically.
    ///
    /// This does not validate field values.
    pub fn new(
        version: i32,
        status: StatusField,
        tick_rate: i32,
        session_info_update: i32,
        session_info_length: i32,
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
            session_info_length,
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

impl Header {
    /// Indicates whether the header is connected.
    pub fn is_connected(&self) -> bool {
        self.status.contains(StatusField::CONNECTED)
    }

    /// Indicates whether the session info has changed compared to `last_update`.
    pub fn session_info_changed(&self, last_update: i32) -> bool {
        self.session_info_update != last_update
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, constants::IRSDK_VER as IRSDK_VERSION};
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
    fn header_layout_matches_iracing_abi() {
        assert_eq!(size_of::<Header>(), 112);

        assert_eq!(align_of::<Header>(), 4);

        assert_eq!(offset_of!(Header, version), 0);
        assert_eq!(offset_of!(Header, status), 4);
        assert_eq!(offset_of!(Header, tick_rate), 8);
        assert_eq!(offset_of!(Header, session_info_update), 12);
        assert_eq!(offset_of!(Header, session_info_length), 16);
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

    #[test]
    fn header_wire_round_trip() {
        let header = valid_live_header();
        let bytes = header.as_bytes();

        let decoded = Header::try_from_bytes(bytes).unwrap();
        assert_eq!(decoded.version, header.version);
        assert_eq!(decoded.status, header.status);
        assert_eq!(
            decoded.variable_header_offset,
            header.variable_header_offset
        );
        assert_eq!(
            decoded.buffers[1].buffer_offset,
            header.buffers[1].buffer_offset
        );
    }

    #[test]
    fn header_from_bytes_rejects_inexact_wire_size() {
        let header = valid_live_header();
        let bytes = header.as_bytes();

        assert!(matches!(
            Header::try_from_bytes(&bytes[..bytes.len() - 1]),
            Err(Error::WireSize {
                expected: 112,
                actual: 111,
            })
        ));
    }

    #[test]
    fn header_reader_rejects_truncated_input() {
        let truncated_data = vec![0u8; 10];
        let mut cursor = std::io::Cursor::new(truncated_data);
        let result = Header::try_from_reader(&mut cursor);

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Parse { .. } => {}
            other => panic!("Expected Parse error, got {:?}", other),
        }
    }
}
