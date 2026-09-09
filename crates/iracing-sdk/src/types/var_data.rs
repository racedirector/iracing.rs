//! Variable data parsing trait and implementations
use super::{BitField, VariableInfo};
use crate::irsdk::VariableType;

/// Trait for types that can be parsed from binary telemetry data.
pub trait VarData: Sized {
    /// Parse this type from binary data at the given offset.
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self>;
}

macro_rules! read_fixed {
    ($data:expr, $info:expr, $variant:ident, $decode:expr $(,)?) => {{
        const EXPECTED: VariableType = VariableType::$variant;

        read_fixed_impl::<
            {
                match EXPECTED.byte_size() {
                    Some(size) => size,
                    None => panic!("expected storage type"),
                }
            },
            _,
        >($data, $info, EXPECTED, $decode)
    }};
}

// Implement VarData for basic types
impl VarData for f32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        read_fixed!(data, info, Float, f32::from_le_bytes)
    }
}

impl VarData for i32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        read_fixed!(data, info, Integer, i32::from_le_bytes)
    }
}

impl VarData for bool {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        read_fixed!(data, info, Boolean, |[byte]| byte != 0)
    }
}

impl VarData for BitField {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        read_fixed!(data, info, BitField, |bytes| {
            BitField(u32::from_le_bytes(bytes))
        })
    }
}

// Additional VarData implementations for all iRacing SDK types
impl VarData for u8 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if !matches!(info.data_type, VariableType::Character) {
            return Err(crate::IRacingSDKError::type_conversion(
                "Character",
                info.data_type,
            ));
        }

        data.get(info.offset)
            .copied()
            .ok_or(crate::IRacingSDKError::memory_access_error(info.offset))
    }
}

impl VarData for f64 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        read_fixed!(data, info, Double, f64::from_le_bytes)
    }
}

// Array support for VarData
impl<T: VarData> VarData for Vec<T> {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        let element_size = info.storage_byte_size()?;
        if info.count == 0 {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(info.count);

        // Clone the variable info and set the count to 1.
        let mut var_info = info.clone();
        // Set the count to 1 to represent a single item within the array.
        var_info.count = 1;

        for i in 0..info.count {
            // Check the offset of the item
            let offset_delta = i
                .checked_mul(element_size)
                .ok_or(crate::IRacingSDKError::memory_access_error(info.offset))?;

            // Set the offset
            var_info.offset = info
                .offset
                .checked_add(offset_delta)
                .ok_or(crate::IRacingSDKError::memory_access_error(info.offset))?;

            // Parse the variable and store it in the result.
            result.push(T::from_bytes(data, &var_info)?);
        }

        Ok(result)
    }
}

#[inline]
fn read_fixed_impl<const SIZE: usize, T>(
    data: &[u8],
    info: &VariableInfo,
    expected: VariableType,
    decode: impl FnOnce([u8; SIZE]) -> T,
) -> crate::Result<T> {
    // Validate we have the right data type
    if info.data_type != expected {
        return Err(crate::IRacingSDKError::type_conversion(
            expected,
            info.data_type,
        ));
    }

    // Read the bytes
    let bytes = data
        .get(info.offset..)
        .and_then(|remaining| remaining.first_chunk::<SIZE>())
        .copied()
        .ok_or(crate::IRacingSDKError::memory_access_error(info.offset))?;

    // Decode and return
    Ok(decode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IRacingSDKError;

    fn variable_info(data_type: VariableType, offset: usize) -> VariableInfo {
        VariableInfo {
            name: "test".to_string(),
            data_type,
            offset,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    fn assert_type_conversion<T: VarData>(data_type: VariableType) {
        assert!(matches!(
            T::from_bytes(&[], &variable_info(data_type, 3)),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
    }

    fn assert_memory<T: VarData>(data_type: VariableType) {
        assert!(matches!(
            T::from_bytes(&[], &variable_info(data_type, 3)),
            Err(IRacingSDKError::Memory { offset: 3, .. })
        ));
    }

    #[test]
    fn fixed_width_scalars_decode_little_endian_at_an_offset() {
        assert_eq!(
            f32::from_bytes(
                &[0, 0, 0, 0x20, 0x41],
                &variable_info(VariableType::Float, 1)
            )
            .unwrap(),
            10.0
        );
        assert_eq!(
            i32::from_bytes(
                &[0, 0x78, 0x56, 0x34, 0x12],
                &variable_info(VariableType::Integer, 1)
            )
            .unwrap(),
            0x1234_5678
        );
        assert!(bool::from_bytes(&[0, 2], &variable_info(VariableType::Boolean, 1)).unwrap());
        assert_eq!(
            BitField::from_bytes(
                &[0, 0x78, 0x56, 0x34, 0x12],
                &variable_info(VariableType::BitField, 1),
            )
            .unwrap()
            .value(),
            0x1234_5678
        );
        assert_eq!(
            f64::from_bytes(
                &[0, 0, 0, 0, 0, 0, 0, 0x24, 0x40],
                &variable_info(VariableType::Double, 1),
            )
            .unwrap(),
            10.0
        );
    }

    #[test]
    fn fixed_width_scalars_check_type_before_reading() {
        assert_type_conversion::<f32>(VariableType::Integer);
        assert_type_conversion::<i32>(VariableType::Float);
        assert_type_conversion::<bool>(VariableType::Character);
        assert_type_conversion::<BitField>(VariableType::Integer);
        assert_type_conversion::<f64>(VariableType::Float);
    }

    #[test]
    fn compatible_fixed_width_scalars_report_memory_for_an_empty_buffer() {
        assert_memory::<f32>(VariableType::Float);
        assert_memory::<i32>(VariableType::Integer);
        assert_memory::<bool>(VariableType::Boolean);
        assert_memory::<BitField>(VariableType::BitField);
        assert_memory::<f64>(VariableType::Double);
    }

    #[test]
    fn u8_accepts_character_and_checks_type_before_reading() {
        assert_eq!(
            u8::from_bytes(&[0, 42], &variable_info(VariableType::Character, 1)).unwrap(),
            42
        );
        assert_eq!(
            u8::from_bytes(&[0, b'x'], &variable_info(VariableType::Character, 1)).unwrap(),
            b'x'
        );
        assert_memory::<u8>(VariableType::Character);
        assert_type_conversion::<u8>(VariableType::Integer);
    }
}
