use type_layout::TypeLayout;

use crate::types::irsdk::wire_type::WireType;

/// iRacing variable buffer information
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
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

unsafe impl WireType for VariableBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    #[test]
    fn variable_buffer_layout_matches_iracing_abi() {
        assert_eq!(VariableBuffer::WIRE_SIZE, 16);

        assert_eq!(align_of::<VariableBuffer>(), 4);

        assert_eq!(offset_of!(VariableBuffer, tick_count), 0);
        assert_eq!(offset_of!(VariableBuffer, buffer_offset), 4);
        assert_eq!(offset_of!(VariableBuffer, tick_count_begin), 8);
        assert_eq!(offset_of!(VariableBuffer, _pad), 12);
    }

    #[test]
    fn variable_buffer_wire_round_trip() {
        let buffer = VariableBuffer::new(10, 20, 9);
        let mut bytes = Vec::new();
        buffer.write_to(&mut bytes).unwrap();
        let decoded = VariableBuffer::read_from_bytes(&bytes).unwrap();
        assert_eq!(decoded.tick_count, 10);
        assert_eq!(decoded.buffer_offset, 20);
        assert_eq!(decoded.tick_count_begin, 9);
    }
}
