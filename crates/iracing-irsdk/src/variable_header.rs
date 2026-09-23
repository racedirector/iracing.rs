use type_layout::TypeLayout;
use zerocopy::{Immutable, IntoBytes, KnownLayout, TryFromBytes};

use crate::parse_utils::try_from_wire_bytes;
use crate::{Error, Result};

use super::VariableType;
use super::constants::{IRSDK_MAX_DESC, IRSDK_MAX_STRING};

/// iRacing variable header structure matching the C SDK layout
#[repr(C)]
#[derive(Debug, Clone, Copy, TypeLayout, TryFromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct VariableHeader {
    /// Variable type (irsdk_VarType enum)
    pub variable_type: VariableType,
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
    /// Decodes one variable header from its exact wire representation.
    ///
    /// This validates that every field has a representable value, including
    /// the [`VariableType`] discriminant. It does not validate relationships
    /// between otherwise representable fields.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WireSize`] when `bytes` is not exactly the size of a
    /// variable header, or [`Error::InvalidWireValue`] when the bytes contain
    /// a value that cannot be represented by a variable header.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        try_from_wire_bytes(bytes)
    }

    /// Constructs a validated variable header and zero-fills its fixed strings and ABI padding.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration error if `offset` is negative, `count`
    /// is not positive, or a string is non-ASCII, contains a NUL, or does not
    /// fit its fixed-width field.
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
        if offset < 0 {
            return Err(Error::invalid_configuration(
                "offset",
                "must be non-negative",
            ));
        }
        if count <= 0 {
            return Err(Error::invalid_configuration(
                "count",
                "must be greater than zero",
            ));
        }

        Ok(Self {
            variable_type,
            offset,
            count,
            count_as_time: u8::from(count_as_time),
            _pad: [0; 3],
            name: fixed_ascii("name", name)?,
            description: fixed_ascii("description", description)?,
            unit: fixed_ascii("unit", unit)?,
        })
    }
}

fn fixed_ascii<const N: usize>(field: &'static str, value: &str) -> Result<[u8; N]> {
    if !value.is_ascii() {
        return Err(Error::invalid_configuration(
            field,
            "must contain ASCII only",
        ));
    }
    if value.as_bytes().contains(&0) {
        return Err(Error::invalid_configuration(
            field,
            "must not contain an interior NUL byte",
        ));
    }
    if value.len() >= N {
        return Err(Error::invalid_configuration(
            field,
            format!("must be shorter than {N} bytes to remain NUL-terminated"),
        ));
    }

    let mut bytes = [0; N];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    fn variable_header_bytes(variable_type: i32) -> [u8; size_of::<VariableHeader>()] {
        let mut bytes = [0; size_of::<VariableHeader>()];
        bytes[0..4].copy_from_slice(&variable_type.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x1020_3040_i32.to_le_bytes());
        bytes[8..12].copy_from_slice(&3_i32.to_le_bytes());
        bytes[12] = 1;
        bytes[13..16].copy_from_slice(&[0xAA, 0xBB, 0xCC]);
        bytes[16..22].copy_from_slice(b"Speed\0");
        bytes[48..62].copy_from_slice(b"Vehicle speed\0");
        bytes[112..116].copy_from_slice(b"m/s\0");
        bytes
    }

    #[test]
    fn variable_header_layout_matches_iracing_abi() {
        assert_eq!(size_of::<VariableHeader>(), 144);
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

        let bytes = header.as_bytes();

        assert_eq!(bytes.len(), size_of::<VariableHeader>());
        assert_eq!(&bytes[0..4], &4i32.to_le_bytes());
        assert_eq!(&bytes[4..8], &8i32.to_le_bytes());
        assert_eq!(&bytes[13..16], &[0; 3]);
        assert_eq!(&bytes[16..22], b"Speed\0");
        assert_eq!(VariableHeader::try_from_bytes(bytes).unwrap().offset, 8);
    }

    #[test]
    fn variable_header_from_bytes_parses_the_complete_wire_representation() {
        let bytes = variable_header_bytes(i32::from(VariableType::Float));

        let header = VariableHeader::try_from_bytes(&bytes).unwrap();

        assert_eq!(header.variable_type, VariableType::Float);
        assert_eq!(header.offset, 0x1020_3040);
        assert_eq!(header.count, 3);
        assert_eq!(header.count_as_time, 1);
        assert_eq!(&header.name[..6], b"Speed\0");
        assert!(header.name[6..].iter().all(|byte| *byte == 0));
        assert_eq!(&header.description[..14], b"Vehicle speed\0");
        assert!(header.description[14..].iter().all(|byte| *byte == 0));
        assert_eq!(&header.unit[..4], b"m/s\0");
        assert!(header.unit[4..].iter().all(|byte| *byte == 0));
        assert_eq!(&header.as_bytes()[13..16], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn variable_header_from_bytes_accepts_every_variable_type_discriminant() {
        for (raw, expected) in [
            (0, VariableType::Character),
            (1, VariableType::Boolean),
            (2, VariableType::Integer),
            (3, VariableType::BitField),
            (4, VariableType::Float),
            (5, VariableType::Double),
        ] {
            let header = VariableHeader::try_from_bytes(&variable_header_bytes(raw)).unwrap();
            assert_eq!(header.variable_type, expected);
        }
    }

    #[test]
    fn variable_header_from_bytes_rejects_inexact_wire_size() {
        let bytes = variable_header_bytes(i32::from(VariableType::Float));

        assert!(matches!(
            VariableHeader::try_from_bytes(&bytes[..bytes.len() - 1]),
            Err(Error::WireSize {
                expected: 144,
                actual: 143,
            })
        ));

        let mut oversized = bytes.to_vec();
        oversized.push(0);
        assert!(matches!(
            VariableHeader::try_from_bytes(&oversized),
            Err(Error::WireSize {
                expected: 144,
                actual: 145,
            })
        ));
    }

    #[test]
    fn variable_header_from_bytes_reports_invalid_variable_type_discriminants() {
        for raw in [-1, 6, 99, i32::MIN, i32::MAX] {
            assert!(matches!(
                VariableHeader::try_from_bytes(&variable_header_bytes(raw)),
                Err(Error::InvalidWireValue { target })
                    if target == std::any::type_name::<VariableHeader>()
            ));
        }
    }

    #[test]
    fn constructor_rejects_invalid_fields() {
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
