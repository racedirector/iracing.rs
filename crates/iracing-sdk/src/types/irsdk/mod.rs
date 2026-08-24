const IRSDK_MAX_BUFFERS: usize = 4;
const IRSDK_MAX_DESC: usize = 64;
const IRSDK_MAX_STRING: usize = 32;
const IRSDK_VERSION: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

mod disk_sub_header;
mod error;
mod header;
mod variable_buffer;
mod variable_header;
mod wire_type;

pub use disk_sub_header::DiskSubHeader;
pub use header::Header;
pub use variable_buffer::VariableBuffer;
pub use variable_header::VariableHeader;
pub use wire_type::WireType;

/// Owned snapshot of the live session YAML region.
#[derive(Debug, Clone)]
pub struct SessionInfoBuffer(Vec<u8>);

impl TryFrom<SessionInfoBuffer> for String {
    type Error = crate::IRacingSDKError;

    fn try_from(buffer: SessionInfoBuffer) -> crate::Result<Self> {
        let length = i32::try_from(buffer.0.len()).map_err(|_| {
            crate::IRacingSDKError::parse_error(
                "SessionInfoBuffer",
                "Session YAML length cannot be represented by the SDK header",
            )
        })?;

        crate::yaml_utils::extract_yaml_from_memory(&buffer.0, 0, length)
    }
}

/// Owned snapshot of a live telemetry frame.
#[derive(Debug, Clone)]
pub struct FrameBuffer(Vec<u8>);

impl From<FrameBuffer> for Vec<u8> {
    fn from(buffer: FrameBuffer) -> Self {
        buffer.0
    }
}

/// Owned snapshot of the live variable header region.
#[derive(Debug, Clone)]
pub struct VariableInfoBuffer {
    pub bytes: Vec<u8>,
    pub count: usize,
}

/// Returns whether the connected status bit is set.
pub fn status_is_connected(status: i32) -> bool {
    status & IRSDK_STATUS_CONNECTED != 0
}
