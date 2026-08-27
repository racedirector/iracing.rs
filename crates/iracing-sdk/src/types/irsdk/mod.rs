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
pub use variable_header::VariableHeadersBuffer;

use crate::{IRacingSDKError, Result, parse_utils};

/// Exact, owned bytes copied from an SDK session-information region.
///
/// The snapshot is source-neutral: live acquisition copies the current mapped
/// region, while IBT acquisition copies the recording's immutable region. The
/// type records ownership of a complete advertised region but does not claim
/// that its contents are valid YAML or correspond atomically to another
/// independently acquired snapshot.
#[derive(Debug, Clone)]
pub struct SessionInfoBuffer {
    /// Complete bytes copied from the advertised session-information region.
    bytes: Vec<u8>,
}

impl SessionInfoBuffer {
    /// Wraps bytes after a reader has copied an advertised region in full.
    ///
    /// Construction is crate-private so source readers remain responsible for
    /// bounds checking and exact-read semantics.
    pub(crate) fn from_snapshot(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl TryFrom<SessionInfoBuffer> for String {
    type Error = IRacingSDKError;

    /// Decodes the snapshot up to its first NUL terminator.
    ///
    /// Valid UTF-8 is preserved. Invalid UTF-8 falls back to a byte-for-byte
    /// single-byte character mapping so later iRacing YAML cleanup retains the
    /// original byte values instead of replacing them.
    fn try_from(buffer: SessionInfoBuffer) -> Result<Self> {
        let yaml_candidate = parse_utils::nul_terminated_bytes(&buffer.bytes);

        if let Ok(s) = std::str::from_utf8(yaml_candidate) {
            Ok(s.to_owned())
        } else {
            Ok(yaml_candidate.iter().map(|&b| b as char).collect())
        }
    }
}

/// Exact, owned bytes for one telemetry frame.
///
/// A `FrameBuffer` contains only wire bytes. Tick count, session version,
/// schema, and source-specific consistency metadata are intentionally attached
/// by later acquisition/provider layers.
#[derive(Debug, Clone)]
pub struct FrameBuffer(Vec<u8>);

impl FrameBuffer {
    /// Wraps bytes after a reader has copied one complete frame region.
    ///
    /// Construction is crate-private so readers remain responsible for
    /// validating the advertised frame length and source bounds.
    pub(crate) fn from_snapshot(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<FrameBuffer> for Vec<u8> {
    /// Releases the owned frame bytes without copying them.
    fn from(buffer: FrameBuffer) -> Self {
        buffer.0
    }
}

/// Returns whether the connected status bit is set.
pub fn status_is_connected(status: i32) -> bool {
    status & IRSDK_STATUS_CONNECTED != 0
}
