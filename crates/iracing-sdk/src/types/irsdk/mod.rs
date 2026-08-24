const IRSDK_MAX_BUFFERS: usize = 4;
const IRSDK_MAX_DESC: usize = 64;
const IRSDK_MAX_STRING: usize = 32;
const IRSDK_VERSION: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

pub mod disk_sub_header;
mod error;
pub mod header;
pub mod variable_buffer;
pub mod variable_header;
mod wire_type;

/// Returns whether the connected status bit is set.
pub(super) fn status_is_connected(status: i32) -> bool {
    status & IRSDK_STATUS_CONNECTED != 0
}
