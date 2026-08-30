//! Variable data parsing trait and implementations

use crate::{IRacingSDKError, Result, parse_utils::decode_variable_type};

use super::{BitField, VariableInfo, VariableType};

/// Trait for types that can be parsed from binary telemetry data.
pub trait VarData: Sized {
    /// Parse this type from binary data at the given offset.
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self>;
}

// Implement VarData for basic types
impl VarData for f32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Float, f32::from_le_bytes)
    }
}

impl VarData for i32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Integer, i32::from_le_bytes)
    }
}

impl VarData for bool {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Boolean, |[byte]| byte != 0)
    }
}

impl VarData for BitField {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, BitField, |bytes| {
            BitField(u32::from_le_bytes(bytes))
        })
    }
}

// Additional VarData implementations for all iRacing SDK types
impl VarData for u8 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        if info.data_type != VariableType::Character {
            return Err(IRacingSDKError::type_conversion(
                VariableType::Character,
                info.data_type,
            ));
        }

        data.get(info.offset)
            .copied()
            .ok_or(IRacingSDKError::memory_access_error(info.offset))
    }
}

impl VarData for f64 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Double, f64::from_le_bytes)
    }
}

// Array support for VarData
impl<T: VarData> VarData for Vec<T> {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        if info.count == 0 {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(info.count);

        // Clone the variable info and set the count to 1.
        let mut var_info = info.clone();
        // Set the count to 1 to represent a single item within the array.
        var_info.count = 1;

        // Cache the size of each element in the array.
        let element_size = info.data_type.byte_size().ok_or_else(|| {
            IRacingSDKError::type_conversion("telemetry storage type", info.data_type)
        })?;

        for i in 0..info.count {
            // Check the offset of the item
            let offset_delta = i
                .checked_mul(element_size)
                .ok_or(IRacingSDKError::memory_access_error(info.offset))?;

            // Set the offset
            var_info.offset = info
                .offset
                .checked_add(offset_delta)
                .ok_or(IRacingSDKError::memory_access_error(info.offset))?;

            // Parse the variable and store it in the result.
            result.push(T::from_bytes(data, &var_info)?);
        }

        Ok(result)
    }
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
            u8::from_bytes(&[0, b'x'], &variable_info(VariableType::Character, 1)).unwrap(),
            b'x'
        );
        assert_memory::<u8>(VariableType::Character);
        assert_type_conversion::<u8>(VariableType::Boolean);
    }
}
