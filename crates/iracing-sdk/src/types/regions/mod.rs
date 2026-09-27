mod bytes;
mod session_info;
mod variable_headers;

pub use {
    bytes::ByteRegion, session_info::SessionInfoRegion, variable_headers::VariableHeadersRegion,
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait ByteParser {
    fn bytes_at_region(&self, region: ByteRegion) -> &[u8];
}
