//! Core runtime telemetry and session types.
mod dynamic_frame;
pub(crate) mod field_data;
mod frame;
mod ibt;
mod iracing_session_string;
mod regions;
mod session_info_bytes;
mod telemetry_layout;
mod telemetry_value;
mod update_rate;
mod variable_headers;

// Re-export all public types
pub use dynamic_frame::DynamicFrame;
pub use field_data::{TelemetryElement, VarData};
pub use frame::FramePacket;
pub use ibt::IbtLayout;
pub use iracing_irsdk::BitField;
pub(crate) use iracing_session_string::IRacingSessionString;
pub use regions::*;
pub use session_info_bytes::{SessionInfoBytes, SessionInfoEncoding, SessionInfoPayload};
pub use telemetry_layout::{FieldId, FieldLayout, FieldMetadata, LayoutProvider, TelemetryLayout};
pub use telemetry_value::{TelemetryValue, TelemetryValueProvider};
pub use update_rate::UpdateRate;
pub use variable_headers::VariableHeaders;

/// Deprecated name for owned session bytes.
#[deprecated(note = "use SessionInfoBytes")]
pub type SessionInfoBuffer = SessionInfoBytes;

/// Deprecated name for owned variable headers.
#[deprecated(note = "use VariableHeaders")]
pub type VariableHeadersBuffer = VariableHeaders;
