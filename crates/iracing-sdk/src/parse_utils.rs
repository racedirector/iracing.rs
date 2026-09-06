//! Utils useful for parsing raw values from buffers

use crate::{IRacingSDKError, Result, VariableInfo, VariableType};

#[allow(unused)]
pub(crate) fn bytes_at_size(data: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;

    data.get(offset..end)
        .ok_or_else(|| IRacingSDKError::memory_access_error(offset))
}

#[allow(unused)]
pub(crate) fn bytes_at<const SIZE: usize>(data: &[u8], offset: usize) -> Result<&[u8; SIZE]> {
    bytes_at_size(data, offset, SIZE)?
        .try_into()
        .map_err(|_| IRacingSDKError::memory_access_error(offset))
}

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

#[allow(unused)]
pub(crate) fn c_string_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(nul_terminated_bytes(bytes)).to_string()
}

#[allow(unused)]
#[inline]
pub(crate) fn decode_bytes_for_variable_info<const SIZE: usize, T>(
    data: &[u8],
    info: &VariableInfo,
    expected: VariableType,
    decode: impl FnOnce([u8; SIZE]) -> T,
) -> Result<T> {
    if info.data_type != expected {
        return Err(IRacingSDKError::type_conversion(expected, info.data_type));
    }

    Ok(decode(*bytes_at::<SIZE>(data, info.offset)?))
}

#[allow(unused)]
/// Decodes a provided `VariableInfo` to it's scalar type.
macro_rules! decode_variable_type {
    ($data:expr, $info:expr, $variant:ident, $decode:expr $(,)?) => {{
        const EXPECTED: $crate::VariableType = $crate::VariableType::$variant;
        const EXPECTED_SIZE: usize = match EXPECTED.byte_size() {
            Some(size) => size,
            None => panic!("telemetry storage type must have a byte size"),
        };

        $crate::parse_utils::decode_bytes_for_variable_info::<EXPECTED_SIZE, _>(
            $data, $info, EXPECTED, $decode,
        )
    }};
}

#[allow(unused)]
pub(crate) use decode_variable_type;
