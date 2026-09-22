//! Utils useful for parsing raw values from buffers

pub(crate) fn nul_terminated_bytes(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

pub(crate) fn c_string_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(nul_terminated_bytes(bytes)).to_string()
}
