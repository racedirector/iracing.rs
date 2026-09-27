//! Utils useful for parsing raw values from buffers

use std::num::NonZeroUsize;

use crate::{ByteRegion, IRacingSDKError, Result, VariableInfo, irsdk::VariableType};

pub(crate) fn bytes_at_size(data: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| IRacingSDKError::memory_invalid_input(offset, length))?;

    data.get(offset..end)
        .ok_or_else(|| IRacingSDKError::memory_unexpected_eof(offset, end, data.len()))
}

pub(crate) fn bytes_at<const SIZE: usize>(data: &[u8], offset: usize) -> Result<&[u8; SIZE]> {
    bytes_at_size(data, offset, SIZE)?
        .try_into()
        .map_err(|e| IRacingSDKError::memory_access_error(offset, e))
}

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

pub(crate) fn c_string_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(nul_terminated_bytes(bytes)).to_string()
}

pub(crate) fn parse_nonnegative_usize(
    field_name: &'static str,
    value: i32,
    context: &'static str,
) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        IRacingSDKError::parse_error(
            context,
            format!("{field_name} must be nonnegative, got {value}"),
        )
    })
}

pub(crate) fn parse_positive_usize(
    field_name: &'static str,
    value: i32,
    context: &'static str,
) -> Result<NonZeroUsize> {
    let value = parse_nonnegative_usize(field_name, value, context)?;
    NonZeroUsize::new(value).ok_or_else(|| {
        IRacingSDKError::parse_error(context, format!("{field_name} must be positive, got 0"))
    })
}

pub(crate) fn validate_metadata_region(
    name: &'static str,
    region: ByteRegion,
    source_len: u64,
    preamble_end: u64,
) -> Result<Option<(u64, u64)>> {
    if region.is_empty() {
        return Ok(None);
    }

    let start = u64::try_from(region.offset()).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT metadata layout",
            format!("{name} offset cannot be represented as a source offset"),
        )
    })?;
    if start < preamble_end {
        return Err(IRacingSDKError::parse_error(
            "IBT metadata layout",
            format!("{name} starts at {start}, before preamble end {preamble_end}"),
        ));
    }

    let end = u64::try_from(region.end()).map_err(|_| {
        IRacingSDKError::parse_error(
            "IBT region bounds",
            "Region end cannot be represented as a source offset",
        )
    })?;
    if end > source_len {
        return Err(IRacingSDKError::parse_error(
            "IBT region bounds",
            format!(
                "Region {}..{} exceeds source length {source_len}",
                region.offset(),
                region.end(),
            ),
        ));
    }

    Ok(Some((start, end)))
}

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

/// Decodes a provided `VariableInfo` to it's scalar type.
macro_rules! decode_variable_type {
    ($data:expr, $info:expr, $variant:ident, $decode:expr $(,)?) => {{
        const EXPECTED: $crate::irsdk::VariableType = $crate::irsdk::VariableType::$variant;
        const EXPECTED_SIZE: usize = EXPECTED.byte_size();

        $crate::parse_utils::decode_bytes_for_variable_info::<EXPECTED_SIZE, _>(
            $data, $info, EXPECTED, $decode,
        )
    }};
}

#[allow(unused)]
pub(crate) use decode_variable_type;
