//! Utils useful for parsing raw values from buffers

use std::num::NonZeroUsize;

use crate::{ByteRegion, IRacingSDKError, Result};

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
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
