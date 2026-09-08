//! Helpers for parsing raw protocol buffers.

use crate::{ProtocolError, Result};

#[allow(unused)]
pub(crate) fn bytes_at_size(data: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| ProtocolError::memory_access_error(offset))?;

    data.get(offset..end)
        .ok_or_else(|| ProtocolError::memory_access_error(offset))
}

#[allow(unused)]
pub(crate) fn bytes_at<const SIZE: usize>(data: &[u8], offset: usize) -> Result<&[u8; SIZE]> {
    bytes_at_size(data, offset, SIZE)?
        .try_into()
        .map_err(|_| ProtocolError::memory_access_error(offset))
}

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    &bytes[..end]
}

#[allow(unused)]
pub(crate) fn c_string_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(nul_terminated_bytes(bytes)).to_string()
}
