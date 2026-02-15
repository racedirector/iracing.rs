//! Frame packet types for stream-based architecture

use std::sync::Arc;

use super::VariableSchema;

/// Raw telemetry frame packet for the stream-based architecture
///
/// This is the fundamental data unit that flows through the system.
/// All other data (adaptations, sessions) is derived from this.
#[derive(Debug, Clone)]
pub struct FramePacket {
    /// Telemetry data buffer (zero-copy via Arc)
    pub data: Arc<[u8]>,

    /// Monotonic frame counter
    pub tick: u32,

    /// Session version (changes trigger session updates)
    pub session_version: u32,

    /// Variable schema for field access
    pub schema: Arc<VariableSchema>,
}

impl FramePacket {
    /// Create a new frame packet
    pub fn new(
        data: Vec<u8>,
        tick: u32,
        session_version: u32,
        schema: Arc<VariableSchema>,
    ) -> Self {
        Self { data: data.into(), tick, session_version, schema }
    }
}
