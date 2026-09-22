//! Conversion of checked telemetry bytes into Rust values.
use super::{BitField, VariableInfo};
use crate::{IRacingSDKError, irsdk::VariableType};

/// Converts one validated telemetry variable into a Rust value.
///
/// [`VariableInfo`] owns type and layout validation. Implementations here only
/// interpret the checked bytes, including the SDK's little-endian encoding.
pub trait VarData: Sized {
    /// Width of one element in the SDK wire format. This must be nonzero and
    /// match every storage type accepted by [`Self::accepts_type`].
    const ELEMENT_SIZE: usize;

    /// Whether this type can represent the SDK storage type.
    fn accepts_type(data_type: VariableType) -> bool;

    /// Storage type expected in type-conversion diagnostics.
    fn expected_type() -> &'static str;

    /// Whether this type represents every element of a variable.
    const IS_ARRAY: bool = false;

    /// Convert a checked variable slice. `data_type` is needed by types that
    /// accept more than one wire representation.
    fn decode_value(bytes: &[u8], data_type: VariableType) -> crate::Result<Self>;

    /// Decode a variable from a frame using its metadata.
    #[inline]
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        info.decode(data)
    }
}

#[inline]
fn fixed_bytes<const N: usize>(bytes: &[u8]) -> crate::Result<[u8; N]> {
    bytes.try_into().map_err(|_| {
        IRacingSDKError::parse_error("VarData::decode_value", "unexpected telemetry byte width")
    })
}

macro_rules! impl_primitive {
    ($type:ty, $variant:ident, $size:expr, $decode:expr) => {
        impl VarData for $type {
            const ELEMENT_SIZE: usize = $size;

            #[inline]
            fn accepts_type(data_type: VariableType) -> bool {
                data_type == VariableType::$variant
            }

            fn expected_type() -> &'static str {
                stringify!($variant)
            }

            #[inline]
            fn decode_value(bytes: &[u8], _data_type: VariableType) -> crate::Result<Self> {
                ($decode)(bytes)
            }
        }
    };
}

// Keep the wire width in one place per implementation; VariableInfo checks
// the corresponding VariableType before these functions are called.
impl_primitive!(f32, Float, 4, |bytes| -> crate::Result<f32> {
    Ok(f32::from_le_bytes(fixed_bytes::<4>(bytes)?))
});
impl_primitive!(i32, Integer, 4, |bytes| -> crate::Result<i32> {
    Ok(i32::from_le_bytes(fixed_bytes::<4>(bytes)?))
});
impl_primitive!(bool, Boolean, 1, |bytes| -> crate::Result<bool> {
    Ok(fixed_bytes::<1>(bytes)?[0] != 0)
});
impl_primitive!(u8, Character, 1, |bytes| -> crate::Result<u8> {
    Ok(fixed_bytes::<1>(bytes)?[0])
});
impl_primitive!(f64, Double, 8, |bytes| -> crate::Result<f64> {
    Ok(f64::from_le_bytes(fixed_bytes::<8>(bytes)?))
});

impl VarData for BitField {
    const ELEMENT_SIZE: usize = 4;

    #[inline]
    fn accepts_type(data_type: VariableType) -> bool {
        data_type == VariableType::BitField
    }

    fn expected_type() -> &'static str {
        "BitField"
    }

    #[inline]
    fn decode_value(bytes: &[u8], _data_type: VariableType) -> crate::Result<Self> {
        Ok(BitField::new(u32::from_le_bytes(fixed_bytes::<4>(bytes)?)))
    }
}

impl<T: VarData> VarData for Vec<T> {
    const ELEMENT_SIZE: usize = T::ELEMENT_SIZE;

    const IS_ARRAY: bool = true;

    fn accepts_type(data_type: VariableType) -> bool {
        T::accepts_type(data_type)
    }

    fn expected_type() -> &'static str {
        T::expected_type()
    }

    #[inline]
    fn decode_value(bytes: &[u8], data_type: VariableType) -> crate::Result<Self> {
        let size = T::ELEMENT_SIZE;
        if size == 0 {
            return Err(IRacingSDKError::parse_error(
                "VarData::decode_value",
                "telemetry element width must be nonzero",
            ));
        }
        let chunks = bytes.chunks_exact(size);
        if !chunks.remainder().is_empty() {
            return Err(IRacingSDKError::parse_error(
                "VarData::decode_value",
                "array byte length is not a multiple of its element width",
            ));
        }

        let mut result = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            result.push(T::decode_value(chunk, data_type)?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IRacingSDKError, irsdk::VariableType};
    use std::fmt::Debug;

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

    fn assert_array_pair<T: VarData + Debug + PartialEq>(
        data_type: VariableType,
        first: &[u8],
        second: &[u8],
        expected: [T; 2],
    ) {
        let mut data = vec![0xAA];
        data.extend_from_slice(first);
        data.extend_from_slice(second);

        let mut info = variable_info(data_type, 1);
        info.count = 2;
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
        let mut info = variable_info(VariableType::Integer, 1);
        info.count = 2;
        assert!(matches!(
            Vec::<u8>::from_bytes(&[], &info),
            Err(IRacingSDKError::TypeConversion { .. })
        ));

        info.data_type = VariableType::Character;
        assert!(matches!(
            Vec::<u8>::from_bytes(&[0xAA, 42], &info),
            Err(IRacingSDKError::Memory { offset: 2, .. })
        ));

        info.data_type = VariableType::Float;
        let mut truncated = vec![0xAA];
        truncated.extend_from_slice(&1.5_f32.to_le_bytes());
        truncated.extend_from_slice(&[0, 0]);
        assert!(matches!(
            Vec::<f32>::from_bytes(&truncated, &info),
            Err(IRacingSDKError::Memory { offset: 5, .. })
        ));
    }

    #[test]
    fn zero_count_array_is_empty_but_invalid_storage_type_is_rejected() {
        let mut info = variable_info(VariableType::Character, usize::MAX);
        info.count = 0;
        assert!(Vec::<u8>::from_bytes(&[], &info).unwrap().is_empty());

        info.data_type = VariableType::Float;
        assert!(matches!(
            info.validate_as::<Vec<u8>>(),
            Err(IRacingSDKError::TypeConversion { .. })
        ));

        info.data_type = VariableType::ElementTypeCount;
        assert!(Vec::<u8>::from_bytes(&[], &info).is_err());
    }

    #[test]
    fn metadata_validation_checks_type_and_scalar_shape_without_frame_data() {
        let mut info = variable_info(VariableType::Float, 1);
        info.count = 2;
        assert!(info.validate_as::<Vec<f32>>().is_ok());
        assert!(matches!(
            info.validate_as::<f32>(),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
        assert!(matches!(
            info.validate_as::<Vec<i32>>(),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
    }

    #[test]
    fn invalid_array_extent_fails_before_allocating_an_output() {
        let mut info = variable_info(VariableType::Float, 0);
        info.count = usize::MAX;
        assert!(matches!(
            info.decode::<Vec<f32>>(&[]),
            Err(IRacingSDKError::Memory { offset: 0, .. })
        ));
    }
}
