use type_layout::TypeLayout;

use crate::{IRacingSDKError, Result};

use super::VariableType;
use super::{
    constants::{IRSDK_MAX_DESC, IRSDK_MAX_STRING},
    error::variable_header_validation_error,
    wire_type::WireType,
};

/// Exact, owned snapshot of a variable-header region advertised by an SDK header.
///
/// Construction is restricted to crate-internal source adapters after they have
/// checked the advertised region and copied or read it in full. Semantic
/// validation of individual headers belongs to later wire-to-domain conversion.
#[derive(Debug, Clone)]
pub struct VariableHeadersBuffer {
    bytes: Vec<u8>,
}

impl VariableHeadersBuffer {
    /// Iterates over the wire headers represented by this exact snapshot.
    pub fn iter_headers(&self) -> impl ExactSizeIterator<Item = VariableHeader> + '_ {
        self.bytes
            .chunks_exact(VariableHeader::WIRE_SIZE)
            .map(|bytes| unsafe { VariableHeader::read_from_bytes_unchecked(bytes) })
    }
}

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
    /// Constructs a validated variable header and zero-fills its fixed strings and ABI padding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        variable_type: VariableType,
        offset: i32,
        count: i32,
        count_as_time: bool,
        name: &str,
        description: &str,
        unit: &str,
    ) -> Result<Self> {
        if !variable_type.is_storage_type() {
            return Err(IRacingSDKError::invalid_configuration(
                "variable_type",
                "ElementTypeCount cannot describe telemetry storage",
            ));
        }
        if offset < 0 {
            return Err(IRacingSDKError::invalid_configuration(
                "offset",
                "must be non-negative",
            ));
        }
        if count <= 0 {
            return Err(IRacingSDKError::invalid_configuration(
                "count",
                "must be greater than zero",
            ));
        }

        Ok(Self {
            variable_type: variable_type.into(),
            offset,
            count,
            count_as_time: u8::from(count_as_time),
            _pad: [0; 3],
            name: fixed_ascii("name", name)?,
            description: fixed_ascii("description", description)?,
            unit: fixed_ascii("unit", unit)?,
        })
    }

    /// Returns the validated SDK storage type advertised by this header.
    pub fn variable_type(&self) -> Result<VariableType> {
        let variable_type = VariableType::try_from(self.variable_type).map_err(|raw| {
            variable_header_validation_error(format!("Unknown variable type value: {raw}"))
        })?;

        if !variable_type.is_storage_type() {
            return Err(variable_header_validation_error(
                "ElementTypeCount cannot describe a telemetry variable",
            ));
        }

        Ok(variable_type)
    }

    /// Validates the semantic fields of a decoded variable header.
    pub fn validate(&self) -> Result<()> {
        self.variable_type()?;

        // Skip empty or invalid variables
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

fn fixed_ascii<const N: usize>(field: &'static str, value: &str) -> Result<[u8; N]> {
    if !value.is_ascii() {
        return Err(IRacingSDKError::invalid_configuration(
            field,
            "must contain ASCII only",
        ));
    }
    if value.as_bytes().contains(&0) {
        return Err(IRacingSDKError::invalid_configuration(
            field,
            "must not contain an interior NUL byte",
        ));
    }
    if value.len() >= N {
        return Err(IRacingSDKError::invalid_configuration(
            field,
            format!("must be shorter than {N} bytes to remain NUL-terminated"),
        ));
    }

    let mut bytes = [0; N];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    Ok(bytes)
}

unsafe impl WireType for VariableHeader {}

impl From<VariableHeadersBuffer> for Vec<VariableHeader> {
    fn from(buffer: VariableHeadersBuffer) -> Self {
        buffer.iter_headers().collect()
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

    #[test]
    fn variable_type_rejects_the_sentinel_and_unknown_values() {
        let mut header = VariableHeader {
            variable_type: i32::from(VariableType::Double),
            offset: 0,
            count: 1,
            count_as_time: 0,
            _pad: [0; 3],
            name: [0; IRSDK_MAX_STRING],
            description: [0; IRSDK_MAX_DESC],
            unit: [0; IRSDK_MAX_STRING],
        };

        assert_eq!(header.variable_type().unwrap(), VariableType::Double);

        header.variable_type = i32::from(VariableType::ElementTypeCount);
        assert!(header.variable_type().is_err());

        header.variable_type = 99;
        assert!(header.variable_type().is_err());
    }

    #[test]
    fn constructor_initializes_exact_wire_representation() {
        let header = VariableHeader::new(
            VariableType::Float,
            8,
            1,
            false,
            "Speed",
            "Vehicle speed",
            "m/s",
        )
        .unwrap();
        let mut bytes = Vec::new();
        header.write_to(&mut bytes).unwrap();

        assert_eq!(bytes.len(), VariableHeader::WIRE_SIZE);
        assert_eq!(&bytes[0..4], &4i32.to_le_bytes());
        assert_eq!(&bytes[4..8], &8i32.to_le_bytes());
        assert_eq!(&bytes[13..16], &[0; 3]);
        assert_eq!(&bytes[16..22], b"Speed\0");
        assert_eq!(VariableHeader::read_from_bytes(&bytes).unwrap().offset, 8);
    }

    #[test]
    fn constructor_rejects_invalid_fields() {
        assert!(
            VariableHeader::new(VariableType::ElementTypeCount, 0, 1, false, "x", "", "").is_err()
        );
        assert!(VariableHeader::new(VariableType::Integer, -1, 1, false, "x", "", "").is_err());
        assert!(VariableHeader::new(VariableType::Integer, 0, 0, false, "x", "", "").is_err());
        assert!(VariableHeader::new(VariableType::Integer, 0, 1, false, "é", "", "").is_err());
        assert!(
            VariableHeader::new(
                VariableType::Integer,
                0,
                1,
                false,
                &"x".repeat(IRSDK_MAX_STRING),
                "",
                ""
            )
            .is_err()
        );
    }
}
