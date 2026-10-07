//! Field-based telemetry decoding, from the canonical runtime layout.
use crate::{BitField, FieldLayout, IRacingSDKError, Result, TelemetryValue, irsdk::VariableType};

/// A scalar element stored in an SDK telemetry field.
///
/// Implementations must decode the advertised SDK width with explicit
/// little-endian semantics. This also enables arrays of domain enum/flag types.
pub trait TelemetryElement: Sized {
    /// Returns whether this Rust type accepts the advertised SDK storage type.
    fn accepts(data_type: VariableType) -> bool;
    /// Decodes one isolated element, rejecting incorrect byte lengths.
    fn decode_element(bytes: &[u8]) -> Result<Self>;
}

/// Decodes a validated field from a telemetry frame.
pub trait VarData: Sized {
    /// Checks storage compatibility and scalar/array shape without reading data.
    fn validate_field(field: &FieldLayout) -> Result<()>;
    /// Validates the requested type and decodes one complete bounded field slice.
    ///
    /// `frame` starts at the beginning of the telemetry frame; `field` supplies
    /// its byte range. Bytes outside that range are ignored. Scalar SDK types
    /// require one element; `Vec<T>` also accepts a single-element field.
    ///
    /// # Errors
    /// Propagates type/shape validation and decoding errors, including a frame
    /// too short to contain the field or an unrecognized domain enum value.
    #[inline]
    fn decode_field(frame: &[u8], field: &FieldLayout) -> Result<Self> {
        Self::validate_field(field)?;
        Self::decode_prevalidated(frame, field)
    }
    /// Decodes after type/shape validation against this exact field.
    ///
    /// This remains bounds checked. Callers must first use `validate_field` and
    /// retain the same field; adapter plans enforce that through layout identity.
    #[doc(hidden)]
    fn decode_prevalidated(frame: &[u8], field: &FieldLayout) -> Result<Self>;
}

/// Borrows the field's frame-relative byte range, returning an unexpected-EOF
/// memory error if the frame does not contain the entire range.
#[inline]
pub(crate) fn field_bytes<'a>(frame: &'a [u8], field: &FieldLayout) -> Result<&'a [u8]> {
    let region = field.region();
    match frame.get(region.as_range()) {
        Some(bytes) => Ok(bytes),
        None => Err(field_out_of_bounds(field, frame.len())),
    }
}

/// Builds the out-of-bounds error away from the hot decoding path.
#[cold]
#[inline(never)]
fn field_out_of_bounds(field: &FieldLayout, frame_len: usize) -> IRacingSDKError {
    let region = field.region();
    IRacingSDKError::memory_unexpected_eof(region.offset(), region.as_region().end(), frame_len)
}

/// Checks the storage type accepted by `T`, returning a type-conversion error
/// on mismatch. Does not check the field's element count.
#[inline]
fn validate_element<T: TelemetryElement>(field: &FieldLayout) -> Result<()> {
    if !T::accepts(field.data_type()) {
        return Err(element_type_mismatch::<T>(field));
    }
    Ok(())
}

/// Builds the storage-type mismatch error away from the hot validation path.
#[cold]
#[inline(never)]
fn element_type_mismatch<T>(field: &FieldLayout) -> IRacingSDKError {
    IRacingSDKError::type_conversion(std::any::type_name::<T>(), field.data_type())
}

/// Builds the scalar-shape mismatch error away from the hot validation path.
#[cold]
#[inline(never)]
fn scalar_count_mismatch(field: &FieldLayout) -> IRacingSDKError {
    IRacingSDKError::type_conversion("scalar count 1", field.count())
}

macro_rules! scalar {
    ($type:ty, $storage:ident, $width:literal, $decode:expr) => {
        impl TelemetryElement for $type {
            #[inline]
            fn accepts(data_type: VariableType) -> bool {
                data_type == VariableType::$storage
            }
            #[inline]
            fn decode_element(bytes: &[u8]) -> Result<Self> {
                let bytes: [u8; $width] =
                    bytes.try_into().map_err(|_| IRacingSDKError::WireSize {
                        expected: $width,
                        actual: bytes.len(),
                    })?;
                Ok(($decode)(bytes))
            }
        }
        impl VarData for $type {
            #[inline]
            fn validate_field(field: &FieldLayout) -> Result<()> {
                validate_element::<Self>(field)?;
                if field.count() != 1 {
                    return Err(scalar_count_mismatch(field));
                }
                Ok(())
            }
            #[inline]
            fn decode_prevalidated(frame: &[u8], field: &FieldLayout) -> Result<Self> {
                Self::decode_element(field_bytes(frame, field)?)
            }
        }
    };
}
scalar!(u8, Character, 1, |[byte]: [u8; 1]| byte);
scalar!(bool, Boolean, 1, |[byte]: [u8; 1]| byte != 0);
scalar!(i32, Integer, 4, i32::from_le_bytes);
scalar!(BitField, BitField, 4, |bytes| BitField(u32::from_le_bytes(
    bytes
)));
scalar!(f32, Float, 4, f32::from_le_bytes);
scalar!(f64, Double, 8, f64::from_le_bytes);

impl<T: TelemetryElement> VarData for Vec<T> {
    #[inline]
    fn validate_field(field: &FieldLayout) -> Result<()> {
        validate_element::<T>(field)
    }
    fn decode_prevalidated(frame: &[u8], field: &FieldLayout) -> Result<Self> {
        let bytes = field_bytes(frame, field)?;
        // Allocate the exact element count once; collecting into `Result<Vec<_>>`
        // cannot use the iterator's size hint and would grow repeatedly.
        let mut values = Vec::with_capacity(field.count());
        for chunk in bytes.chunks_exact(field.data_type().byte_size()) {
            values.push(T::decode_element(chunk)?);
        }
        Ok(values)
    }
}

impl TelemetryValue {
    /// Decodes a selected field with one bounded slice and explicit little-endian values.
    ///
    /// Returns a scalar for a one-element field and an array otherwise. Character
    /// fields retain their byte values; any nonzero boolean byte becomes `true`.
    /// `frame` starts at the beginning of the telemetry frame.
    ///
    /// # Errors
    /// Returns an unexpected-EOF memory error if `frame` does not contain the
    /// field's full byte range.
    pub fn decode_field(frame: &[u8], field: &FieldLayout) -> Result<Self> {
        let bytes = field_bytes(frame, field)?;
        fn values<T: TelemetryElement>(
            bytes: &[u8],
            field: &FieldLayout,
            wrap: fn(T) -> TelemetryValue,
        ) -> Result<TelemetryValue> {
            if field.count() == 1 {
                return T::decode_element(bytes).map(wrap);
            }
            // Preallocate exactly; `collect::<Result<Vec<_>>>` would grow repeatedly.
            let mut values = Vec::with_capacity(field.count());
            for chunk in bytes.chunks_exact(field.data_type().byte_size()) {
                values.push(wrap(T::decode_element(chunk)?));
            }
            Ok(TelemetryValue::Array(values))
        }
        match field.data_type() {
            VariableType::Character => values(bytes, field, Self::Char),
            VariableType::Boolean => values(bytes, field, Self::Bool),
            VariableType::Integer => values(bytes, field, Self::Int32),
            VariableType::BitField => values(bytes, field, Self::BitField),
            VariableType::Float => values(bytes, field, Self::Float32),
            VariableType::Double => values(bytes, field, Self::Float64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::VariableHeader;
    fn field(storage: VariableType, count: i32, width: usize) -> FieldLayout {
        let header = VariableHeader::new(storage, 1, count, false, "Storage", "", "").unwrap();
        FieldLayout::try_from_header(&header, 1 + count as usize * width).unwrap()
    }
    #[test]
    fn all_scalar_types_and_truncated_frames() {
        // Scalars and arrays use synthetic nonzero-offset geometry to exercise widths.
        check_scalar::<u8>(
            VariableType::Character,
            &[0xff],
            0xff,
            TelemetryValue::Char(0xff),
        );
        check_scalar::<bool>(
            VariableType::Boolean,
            &[2],
            true,
            TelemetryValue::Bool(true),
        );
        check_scalar::<i32>(
            VariableType::Integer,
            &[0x78, 0x56, 0x34, 0x12],
            0x12345678,
            TelemetryValue::Int32(0x12345678),
        );
        check_scalar::<BitField>(
            VariableType::BitField,
            &[0x78, 0x56, 0x34, 0x12],
            BitField(0x12345678),
            TelemetryValue::BitField(BitField(0x12345678)),
        );
        check_scalar::<f32>(
            VariableType::Float,
            &[0, 0, 0x20, 0x41],
            10.0,
            TelemetryValue::Float32(10.0),
        );
        check_scalar::<f64>(
            VariableType::Double,
            &[0, 0, 0, 0, 0, 0, 0x24, 0x40],
            10.0,
            TelemetryValue::Float64(10.0),
        );
    }
    fn check_scalar<T: VarData + TelemetryElement + PartialEq + std::fmt::Debug + Clone>(
        storage: VariableType,
        bytes: &[u8],
        expected: T,
        dynamic: TelemetryValue,
    ) {
        let scalar = field(storage, 1, bytes.len());
        let mut frame = vec![0xff];
        frame.extend_from_slice(bytes);
        assert_eq!(T::decode_field(&frame, &scalar).unwrap(), expected);
        assert_eq!(
            TelemetryValue::decode_field(&frame, &scalar).unwrap(),
            dynamic
        );
        assert!(T::decode_field(&frame[..frame.len() - 1], &scalar).is_err());
        let array = field(storage, 2, bytes.len());
        frame.extend_from_slice(bytes);
        assert_eq!(
            Vec::<T>::decode_field(&frame, &array).unwrap(),
            vec![expected.clone(), expected]
        );
        assert_eq!(
            TelemetryValue::decode_field(&frame, &array).unwrap(),
            TelemetryValue::Array(vec![dynamic.clone(), dynamic])
        );
        assert!(Vec::<T>::decode_field(&frame[..frame.len() - 1], &array).is_err());
        assert!(TelemetryValue::decode_field(&frame[..frame.len() - 1], &array).is_err());
        assert!(T::validate_field(&array).is_err());
    }
    #[test]
    fn wrong_type_fails_before_reading() {
        let field = field(VariableType::Float, 1, 4);
        assert!(matches!(
            i32::decode_field(&[], &field),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
        assert!(matches!(
            Vec::<bool>::decode_field(&[], &field),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn scalar_bit_patterns_round_trip(bits in any::<u32>(), offset in 0..100usize) {
            let mut data = vec![0; offset + 4];
            data[offset..].copy_from_slice(&bits.to_le_bytes());
            for storage in [VariableType::Integer, VariableType::Float, VariableType::BitField] {
                let field = crate::test_utils::field("Speed".into(), storage, offset, 1, false, String::new(), String::new());
                let decoded = match storage {
                    VariableType::Integer => i32::decode_field(&data, &field).unwrap() as u32,
                    VariableType::Float => f32::decode_field(&data, &field).unwrap().to_bits(),
                    VariableType::BitField => BitField::decode_field(&data, &field).unwrap().value(),
                    _ => unreachable!(),
                };
                prop_assert_eq!(decoded, bits);
                prop_assert!(TelemetryValue::decode_field(&data[..data.len()-1], &field).is_err());
            }
        }
        #[test]
        fn integer_arrays_round_trip(values in prop::collection::vec(any::<i32>(), 1..73), offset in 0..100usize) {
            let mut data = vec![0; offset];
            for value in &values { data.extend(value.to_le_bytes()); }
            let field = crate::test_utils::field("CarIdxLap".into(), VariableType::Integer, offset, values.len(), false, String::new(), String::new());
            prop_assert_eq!(Vec::<i32>::decode_field(&data, &field).unwrap(), values);
            prop_assert!(Vec::<i32>::decode_field(&data[..data.len()-1], &field).is_err());
        }
    }
}
