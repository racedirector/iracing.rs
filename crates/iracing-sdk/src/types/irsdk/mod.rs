//! Rust representations of definitions from `irsdk_defines.h`.
//!
//! The SDK declarations are modeled here independently of the crate's existing
//! domain-facing telemetry and broadcast types.

const IRSDK_MAX_BUFFERS: usize = 4;
const IRSDK_MAX_DESC: usize = 64;
const IRSDK_MAX_STRING: usize = 32;
/// Wire-format version supported by this SDK implementation.
pub const IRSDK_VERSION: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry.
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

mod macros;

pub mod broadcast;
pub mod constants;
pub mod flags;
pub mod telemetry;
pub mod variable_type;

// Module API
mod disk_sub_header;
mod error;
mod header;
mod variable_buffer;
mod variable_header;
mod wire_type;

// Existing public API
pub use disk_sub_header::DiskSubHeader;
pub use header::Header;
pub use variable_buffer::VariableBuffer;
pub use variable_header::VariableHeader;
pub use wire_type::WireType;

// SDK definition API
pub use broadcast::{
    BroadcastMessage, CameraSwitchFocusMode, ChatCommandMode, ForceFeedbackCommandMode,
    PitCommandMode, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode,
    TelemetryCommandMode, VideoCaptureMode,
};
pub use flags::{
    CameraState, EngineWarnings, IncidentFlags, PaceFlags, PitServiceFlags, SessionFlags,
    StatusField,
};
pub use telemetry::{
    CarLeftRight, PaceMode, PitServiceStatus, SessionState, TrackLocation, TrackSurface,
    TrackWetness,
};
pub use variable_type::VariableType;

// Crate-public API
pub(crate) use variable_header::VariableHeadersBuffer;

use crate::{IRacingSDKError, Result, yaml_utils};

/// Owned snapshot of the live session YAML region.
#[derive(Debug, Clone)]
pub struct SessionInfoBuffer(pub(crate) Vec<u8>);

impl TryFrom<SessionInfoBuffer> for String {
    type Error = IRacingSDKError;

    fn try_from(buffer: SessionInfoBuffer) -> Result<Self> {
        let length = i32::try_from(buffer.0.len()).map_err(|_| {
            IRacingSDKError::parse_error(
                "SessionInfoBuffer",
                "Session YAML length cannot be represented by the SDK header",
            )
        })?;

        yaml_utils::extract_yaml_from_memory(&buffer.0, 0, length)
    }
}

/// Owned snapshot of a live telemetry frame.
#[derive(Debug, Clone)]
pub struct FrameBuffer(pub(crate) Vec<u8>);

impl From<FrameBuffer> for Vec<u8> {
    fn from(buffer: FrameBuffer) -> Self {
        buffer.0
    }
}

/// Returns whether the connected status bit is set.
pub fn status_is_connected(status: i32) -> bool {
    status & IRSDK_STATUS_CONNECTED != 0
}
