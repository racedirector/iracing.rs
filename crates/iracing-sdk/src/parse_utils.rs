//! Utils useful for parsing raw values from buffers

use crate::{IRacingSDKError, VariableInfo, VariableType};

pub(crate) fn bytes_at<const SIZE: usize>(
    data: &[u8],
    offset: usize,
) -> crate::Result<&[u8; SIZE]> {
    let end = offset
        .checked_add(SIZE)
        .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;

    data.get(offset..end)
        .and_then(|slice| slice.first_chunk::<SIZE>())
        .ok_or_else(|| IRacingSDKError::memory_access_error(offset))
}

#[inline]
pub(crate) fn decode_bytes_for_variable_info<const SIZE: usize, T>(
    data: &[u8],
    info: &VariableInfo,
    expected: VariableType,
    decode: impl FnOnce([u8; SIZE]) -> T,
) -> crate::Result<T> {
    if info.data_type != expected {
        return Err(IRacingSDKError::type_conversion(expected, info.data_type));
    }

    Ok(decode(*bytes_at::<SIZE>(data, info.offset)?))
}

/// Decodes a provided `VariableInfo` to it's scalar type.
macro_rules! decode_variable_type {
    ($data:expr, $info:expr, $variant:ident, $decode:expr $(,)?) => {{
        const EXPECTED: VariableType = VariableType::$variant;

        $crate::parse_utils::decode_bytes_for_variable_info::<{ EXPECTED.size() }, _>(
            $data, $info, EXPECTED, $decode,
        )
    }};
}

pub(crate) use decode_variable_type;
