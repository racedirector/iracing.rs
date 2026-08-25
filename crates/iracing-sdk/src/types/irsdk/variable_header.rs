use type_layout::TypeLayout;

use crate::{IRacingSDKError, Result};

use super::{
    IRSDK_MAX_DESC, IRSDK_MAX_STRING, VariableInfoBuffer, error::variable_header_validation_error,
    wire_type::WireType,
};

/// iRacing variable header structure matching the C SDK layout
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout)]
pub struct VariableHeader {
    /// Variable type (irsdk_VarType enum)
    pub variable_type: i32,
    /// Offset in bytes from buffer start
    pub offset: i32,
    /// Number of elements (1 for scalar, >1 for arrays)
    pub count: i32,
    /// Whether the count field should be interpreted as time
    pub count_as_time: u8,
    /// Padding for alignment (matches 3-byte C padding)
    _pad: [u8; 3],
    /// Variable name (32 bytes, null-terminated)
    pub name: [u8; IRSDK_MAX_STRING],
    /// Variable description (64 bytes, null-terminated)
    pub description: [u8; IRSDK_MAX_DESC],
    /// Variable units (32 bytes, null-terminated)
    pub unit: [u8; IRSDK_MAX_STRING],
}

impl VariableHeader {
    /// Validates the semantic fields of a decoded variable header.
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(variable_header_validation_error("Name cannot be empty"));
        }

        if self.offset < 0 {
            return Err(variable_header_validation_error(
                "Offset cannot be negative",
            ));
        }

        if self.count <= 0 {
            return Err(variable_header_validation_error(
                "Count cannot be 0 or less",
            ));
        }

        Ok(())
    }
}

unsafe impl WireType for VariableHeader {}

impl TryFrom<VariableInfoBuffer> for Vec<VariableHeader> {
    type Error = IRacingSDKError;

    fn try_from(buffer: VariableInfoBuffer) -> Result<Self, Self::Error> {
        debug_assert_eq!(buffer.bytes.len(), buffer.count * VariableHeader::WIRE_SIZE);

        buffer
            .bytes
            .chunks_exact(VariableHeader::WIRE_SIZE)
            .map(VariableHeader::read_from_bytes)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    #[test]
    fn variable_header_layout_matches_iracing_abi() {
        assert_eq!(VariableHeader::WIRE_SIZE, 144);
        assert_eq!(align_of::<VariableHeader>(), 4);

        assert_eq!(offset_of!(VariableHeader, variable_type), 0);
        assert_eq!(offset_of!(VariableHeader, offset), 4);
        assert_eq!(offset_of!(VariableHeader, count), 8);
        assert_eq!(offset_of!(VariableHeader, count_as_time), 12);
        assert_eq!(offset_of!(VariableHeader, _pad), 13);
        assert_eq!(offset_of!(VariableHeader, name), 16);
        assert_eq!(offset_of!(VariableHeader, description), 48);
        assert_eq!(offset_of!(VariableHeader, unit), 112);
    }
}
