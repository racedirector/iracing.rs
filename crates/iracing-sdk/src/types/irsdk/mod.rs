//! Rust representations of definitions from `irsdk_defines.h`.
//!
//! The SDK declarations are modeled here independently of the crate's existing
//! domain-facing telemetry and broadcast types.

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
mod iracing_session_string;
mod session_info_buffer;
mod variable_buffer;
mod variable_header;
mod wire_type;

// Existing public API
// pub use disk_sub_header::DiskSubHeader;
// pub use header::Header;
// pub use variable_buffer::VariableBuffer;
// pub use variable_header::VariableHeader;
pub use wire_type::WireType;

// SDK definition API
// pub use broadcast::{
//     BroadcastMessage, CameraSwitchFocusMode, ChatCommandMode, ForceFeedbackCommandMode,
//     PitCommandMode, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode,
//     TelemetryCommandMode, VideoCaptureMode,
// };
// pub use flags::{
//     CameraState, EngineWarnings, IncidentFlags, PaceFlags, PitServiceFlags, SessionFlags,
//     StatusField,
// };
// pub(crate) use iracing_session_string::IRacingSessionString;
// pub use session_info_buffer::SessionInfoBuffer;
// pub use telemetry::{
//     CarLeftRight, PaceMode, PitServiceStatus, SessionState, TrackLocation, TrackSurface,
//     TrackWetness,
// };
pub use variable_type::VariableType;

// Crate-public API
// pub use variable_header::VariableHeadersBuffer;
