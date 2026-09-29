//! Variable data parsing trait and implementations
use super::{BitField, VariableInfo};
use crate::{IRacingSDKError, Result, parse_utils::decode_variable_type};

/// Trait for types that can be parsed from binary telemetry data.
pub trait VarData: Sized {
    /// Parse this type from binary data at the given offset.
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self>;
}

/// irsdk::VariableType::Float
impl VarData for f32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Float, f32::from_le_bytes)
    }
}

/// irsdk::VariableType::Integer
impl VarData for i32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Integer, i32::from_le_bytes)
    }
}

/// irsdk::VariableType::Bool
impl VarData for bool {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Boolean, |[byte]| byte != 0)
    }
}

/// irsdk::VariableType::BitField
impl VarData for BitField {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, BitField, |bytes| {
            BitField(u32::from_le_bytes(bytes))
        })
    }
}

/// irsdk::VariableType::Character
impl VarData for u8 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Character, |[byte]| byte)
    }
}

/// irsdk::VariableType::Double
impl VarData for f64 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        decode_variable_type!(data, info, Double, f64::from_le_bytes)
    }
}

// Array support for VarData
impl<T: VarData> VarData for Vec<T> {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> Result<Self> {
        let element_size = info.data_type().byte_size();
        let mut result = Vec::with_capacity(info.count());
        let mut scalar_info = info.clone();
        let region_end = info.region().as_region().end();

        for i in 0..info.count() {
            // Check the offset of the item
            let offset_delta = i
                .checked_mul(element_size)
                .ok_or_else(|| {
                    IRacingSDKError::parse_error(
                        "VarData::from_bytes",
                        format!(
                            "Array element {i} offset calculation overflows usize for element size {element_size}"
                        ),
                    )
                })?;

            // Set the offset
            let offset = info.offset().checked_add(offset_delta).ok_or_else(|| {
                IRacingSDKError::memory_invalid_input(info.offset(), offset_delta)
            })?;

            // Parse the variable and store it in the result.
            scalar_info.set_scalar_offset(offset, region_end)?;
            result.push(T::from_bytes(data, &scalar_info)?);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::VariableType;
    use std::fmt::Debug;

    fn variable_info(data_type: VariableType, offset: usize) -> VariableInfo {
        variable_info_with_count(data_type, offset, 1)
    }

    fn variable_info_with_count(
        data_type: VariableType,
        offset: usize,
        count: usize,
    ) -> VariableInfo {
        let frame_size = offset + data_type.byte_size() * count;
        VariableInfo::try_new(
            "test".to_string(),
            data_type,
            offset,
            count,
            frame_size,
            false,
            String::new(),
            String::new(),
        )
        .unwrap()
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

    fn assert_array_pair<T: VarData + Debug + PartialEq>(
        data_type: VariableType,
        first: &[u8],
        second: &[u8],
        expected: [T; 2],
    ) {
        let mut data = vec![0xAA];
        data.extend_from_slice(first);
        data.extend_from_slice(second);

        let info = variable_info_with_count(data_type, 1, 2);
        assert_eq!(Vec::<T>::from_bytes(&data, &info).unwrap(), expected);
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
        assert_type_conversion::<u8>(VariableType::Integer);
    }

    #[test]
    fn compatible_fixed_width_scalars_report_memory_for_an_empty_buffer() {
        assert_memory::<f32>(VariableType::Float);
        assert_memory::<i32>(VariableType::Integer);
        assert_memory::<bool>(VariableType::Boolean);
        assert_memory::<BitField>(VariableType::BitField);
        assert_memory::<f64>(VariableType::Double);
        assert_memory::<u8>(VariableType::Character);
    }

    #[test]
    fn character_decoding_preserves_raw_byte_values() {
        assert_eq!(
            u8::from_bytes(&[0xAA, 0], &variable_info(VariableType::Character, 1)).unwrap(),
            0
        );
        assert_eq!(
            u8::from_bytes(&[0xAA, 0xFF], &variable_info(VariableType::Character, 1)).unwrap(),
            0xFF
        );
    }

    #[test]
    fn arrays_decode_every_storage_type_at_a_nonzero_offset() {
        assert_array_pair(VariableType::Character, &[0], &[0xFF], [0_u8, 0xFF]);
        assert_array_pair(VariableType::Boolean, &[0], &[2], [false, true]);
        assert_array_pair(
            VariableType::Integer,
            &(-2_i32).to_le_bytes(),
            &0x1234_5678_i32.to_le_bytes(),
            [-2_i32, 0x1234_5678],
        );
        assert_array_pair(
            VariableType::Float,
            &(-1.5_f32).to_le_bytes(),
            &10.25_f32.to_le_bytes(),
            [-1.5_f32, 10.25_f32],
        );
        assert_array_pair(
            VariableType::Double,
            &(-1.5_f64).to_le_bytes(),
            &10.25_f64.to_le_bytes(),
            [-1.5_f64, 10.25_f64],
        );
        assert_array_pair(
            VariableType::BitField,
            &0x8000_0000_u32.to_le_bytes(),
            &0x1234_5678_u32.to_le_bytes(),
            [BitField::new(0x8000_0000), BitField::new(0x1234_5678)],
        );
    }

    #[test]
    fn arrays_report_type_mismatch_and_later_element_bounds_errors() {
        let info = variable_info_with_count(VariableType::Integer, 1, 2);
        assert!(matches!(
            Vec::<u8>::from_bytes(&[], &info),
            Err(IRacingSDKError::TypeConversion { .. })
        ));

        let info = variable_info_with_count(VariableType::Character, 1, 2);
        assert!(matches!(
            Vec::<u8>::from_bytes(&[0xAA, 42], &info),
            Err(IRacingSDKError::Memory { offset: 2, .. })
        ));

        let info = variable_info_with_count(VariableType::Float, 1, 2);
        let mut truncated = vec![0xAA];
        truncated.extend_from_slice(&1.5_f32.to_le_bytes());
        truncated.extend_from_slice(&[0, 0]);
        assert!(matches!(
            Vec::<f32>::from_bytes(&truncated, &info),
            Err(IRacingSDKError::Memory { offset: 5, .. })
        ));
    }

    #[test]
    fn zero_count_metadata_is_rejected() {
        assert!(
            VariableInfo::try_new(
                "test".into(),
                VariableType::Character,
                0,
                0,
                0,
                false,
                String::new(),
                String::new(),
            )
            .is_err()
        );
    }
}
