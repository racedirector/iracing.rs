use std::{io::Read, ops::Range};

use super::{StatusField, VariableBuffer, constants::IRSDK_MAX_BUFS as IRSDK_MAX_BUFFERS};
use crate::{
    Result, VariableHeader,
    parse_utils::{read_wire_bytes, read_wire_bytes_from_io},
};

/// An iRacing SDK header.
#[repr(C)]
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "debug", derive(type_layout::TypeLayout))]
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
    #[cfg_attr(feature = "serde", serde(skip))]
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
    /// Returns [`crate::Error::Io`] if `reader` cannot supply a complete header.
    pub fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        read_wire_bytes_from_io(reader)
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

    /// Indicates whether the header is connected.
    pub fn is_connected(&self) -> bool {
        self.status.contains(StatusField::CONNECTED)
    }

    /// Indicates whether the session info has changed compared to `last_update`.
    pub fn session_info_changed(&self, last_update: i32) -> bool {
        self.session_info_update != last_update
    }

    /// Returns the range of the session info buffer, relative to file start.
    /// Returns `None` if the range endpoint overflows.
    pub fn session_info_range(&self) -> Option<Range<i32>> {
        i32_checked_range(self.session_info_offset, self.session_info_length)
    }

    /// Returns the range of the variable header array, relative to file start.
    /// Returns `None` if the range endpoint overflows.
    pub fn variable_headers_range(&self) -> Option<Range<i32>> {
        let length = self
            .variable_count
            .checked_mul(size_of::<VariableHeader>() as i32)?;

        i32_checked_range(self.variable_header_offset, length)
    }

    /// Returns the advertised buffer descriptor at `index`.
    ///
    /// Returns `None` if the advertised buffer count is invalid or `index` is
    /// outside that count. This does not validate or copy the buffer's frame data.
    pub fn variable_buffer(&self, index: usize) -> Option<&VariableBuffer> {
        let count = usize::try_from(self.buffer_count).ok()?;
        if count == 0 || count > Self::MAX_BUFFERS || index >= count {
            return None;
        }

        self.buffers.get(index)
    }

    /// Returns the most recently published buffer descriptor.
    ///
    /// Returns `None` if the advertised buffer count or current index is
    /// invalid. A live writer may reuse this buffer after it is selected, so
    /// callers must check for a torn read when copying its frame data.
    pub fn current_variable_buffer(&self) -> Option<&VariableBuffer> {
        self.variable_buffer(usize::from(self.current_buffer))
    }

    /// Returns the range of the buffer at `index`.
    pub fn variable_buffer_range(&self, index: usize) -> Option<Range<i32>> {
        let buffer = self.variable_buffer(index)?;
        i32_checked_range(buffer.buffer_offset, self.buffer_length)
    }

    /// Returns the range of the most recently published buffer.
    pub fn current_variable_buffer_range(&self) -> Option<Range<i32>> {
        let buffer = self.current_variable_buffer()?;
        i32_checked_range(buffer.buffer_offset, self.buffer_length)
    }
}

/// ???: Consider implementing a macro to generate the helpers...
fn i32_checked_range(offset: i32, length: i32) -> Option<Range<i32>> {
    let end = offset.checked_add(length)?;
    Some(offset..end)
}

// Native SDK ABI invariants must hold even when tests are not built.
const _: () = {
    assert!(std::mem::size_of::<Header>() == 112);
    assert!(std::mem::align_of::<Header>() == 4);
    assert!(std::mem::offset_of!(Header, version) == 0);
    assert!(std::mem::offset_of!(Header, status) == 4);
    assert!(std::mem::offset_of!(Header, tick_rate) == 8);
    assert!(std::mem::offset_of!(Header, session_info_update) == 12);
    assert!(std::mem::offset_of!(Header, session_info_length) == 16);
    assert!(std::mem::offset_of!(Header, session_info_offset) == 20);
    assert!(std::mem::offset_of!(Header, variable_count) == 24);
    assert!(std::mem::offset_of!(Header, variable_header_offset) == 28);
    assert!(std::mem::offset_of!(Header, buffer_count) == 32);
    assert!(std::mem::offset_of!(Header, buffer_length) == 36);
    assert!(std::mem::offset_of!(Header, current_buffer_tick_count) == 40);
    assert!(std::mem::offset_of!(Header, current_buffer) == 44);
    assert!(std::mem::offset_of!(Header, _pad) == 45);
    assert!(std::mem::offset_of!(Header, buffers) == 48);
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, constants::IRSDK_VER as IRSDK_VERSION};
    use std::mem::{align_of, offset_of};
    use zerocopy::IntoBytes;

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
    fn ranges_return_none_when_length_or_endpoint_overflows() {
        let mut header = valid_live_header();
        assert_eq!(header.session_info_range(), Some(112..1_112));
        assert_eq!(header.variable_headers_range(), Some(1_112..15_512));

        header.session_info_offset = i32::MAX;
        assert_eq!(header.session_info_range(), None);

        header.variable_count = i32::MAX;
        assert_eq!(header.variable_headers_range(), None);

        header.variable_count = 1;
        header.variable_header_offset = i32::MAX;
        assert_eq!(header.variable_headers_range(), None);
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
    fn buffer_accessors_respect_advertised_count() {
        let mut header = valid_live_header();
        header.buffer_count = 2;
        header.current_buffer = 1;

        assert_eq!(
            header.variable_buffer(0).map(|buffer| buffer.tick_count),
            Some(10)
        );
        assert_eq!(
            header.variable_buffer(1).map(|buffer| buffer.tick_count),
            Some(9)
        );
        assert!(header.variable_buffer(2).is_none());
        assert_eq!(
            header
                .current_variable_buffer()
                .map(|buffer| buffer.tick_count),
            Some(9)
        );

        header.current_buffer = 2;
        assert!(header.current_variable_buffer().is_none());
    }

    #[test]
    fn buffer_accessors_reject_invalid_advertised_counts() {
        let mut header = valid_live_header();

        for count in [-1, 0, Header::MAX_BUFFERS as i32 + 1] {
            header.buffer_count = count;
            assert!(header.variable_buffer(0).is_none());
            assert!(header.current_variable_buffer().is_none());
        }
    }

    #[test]
    fn header_reader_reports_truncated_input_as_io_error() {
        let truncated_data = vec![0u8; 10];
        let mut cursor = std::io::Cursor::new(truncated_data);
        let result = Header::try_from_reader(&mut cursor);

        assert!(matches!(
            result,
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof
        ));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serialization_round_trip_without_padding() {
        let header = valid_live_header();

        let header_json = serde_json::to_value(header).unwrap();
        assert!(header_json.get("_pad").is_none());
        assert!(header_json["buffers"][0].get("_pad").is_none());
        let decoded_header: Header = serde_json::from_value(header_json).unwrap();
        assert_eq!(decoded_header.as_bytes(), header.as_bytes());
    }
}
