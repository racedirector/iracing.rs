mod bytes;
mod frame;
mod frames;
mod session_info;
mod variable;
mod variable_headers;

pub use {
    bytes::ByteRegion, frame::FrameRegion, frames::FramesRegion, session_info::SessionInfoRegion,
    variable::VariableRegion, variable_headers::VariableHeadersRegion,
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait ByteParser {
    fn bytes_at_region(&self, region: ByteRegion) -> &[u8];
}
