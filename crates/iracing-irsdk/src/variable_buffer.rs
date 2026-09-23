use type_layout::TypeLayout;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{Result, parse_utils::read_wire_bytes};

/// iRacing variable buffer information
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct VariableBuffer {
    /// Tick count when buffer was written
    pub tick_count: i32,
    /// Offset from header to buffer start
    pub buffer_offset: i32,
    /// Tick count written before a frame write begins, used for torn-read detection
    pub tick_count_begin: i32,
    /// Padding to maintain alignment
    _pad: [i32; 1],
}

impl VariableBuffer {
    /// Decodes one variable-buffer descriptor from its exact wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::WireSize`] when `bytes` is not exactly the size
    /// of a variable-buffer descriptor.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        read_wire_bytes(bytes)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn variable_buffer_layout_matches_iracing_abi() {
        assert_eq!(size_of::<VariableBuffer>(), 16);

        assert_eq!(align_of::<VariableBuffer>(), 4);

        assert_eq!(offset_of!(VariableBuffer, tick_count), 0);
        assert_eq!(offset_of!(VariableBuffer, buffer_offset), 4);
        assert_eq!(offset_of!(VariableBuffer, tick_count_begin), 8);
        assert_eq!(offset_of!(VariableBuffer, _pad), 12);
    }

    #[test]
    fn variable_buffer_wire_round_trip() {
        let buffer = VariableBuffer::new(10, 20, 9);
        let bytes = buffer.as_bytes();

        let decoded = VariableBuffer::try_from_bytes(bytes).unwrap();
        assert_eq!(decoded.tick_count, 10);
        assert_eq!(decoded.buffer_offset, 20);
        assert_eq!(decoded.tick_count_begin, 9);
    }

    #[test]
    fn variable_buffer_from_bytes_rejects_inexact_wire_size() {
        let buffer = VariableBuffer::new(10, 20, 9);
        let bytes = buffer.as_bytes();

        assert!(matches!(
            VariableBuffer::try_from_bytes(&bytes[..bytes.len() - 1]),
            Err(crate::Error::WireSize {
                expected: 16,
                actual: 15,
            })
        ));
    }
}
