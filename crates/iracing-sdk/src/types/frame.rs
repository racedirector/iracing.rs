//! Frame packet types for stream-based architecture

use std::sync::Arc;

use crate::{
    TelemetryValue, VariableInfo, VariableSchema, types::variable_type::TelemetryValueProvider,
};

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
        Self {
            data: data.into(),
            tick,
            session_version,
            schema,
        }
    }
}

impl FramePacket {
    /// Returns variable metadata if present.
    pub fn variable_info(&self, name: &str) -> Option<&VariableInfo> {
        self.schema.variables.get(name)
    }

    /// Retrieves the variable from the frame by name.
    pub fn value(
        &self,
        name: &str,
    ) -> crate::Result<Option<crate::types::variable_type::TelemetryValue>> {
        let Some(info) = self.variable_info(name) else {
            return Ok(None);
        };

        self.telemetry_value_from_info(info).map(Some)
    }
}

impl TelemetryValueProvider for FramePacket {
    fn telemetry_value_from_info(&self, info: &VariableInfo) -> crate::Result<TelemetryValue> {
        TelemetryValue::decode(self.data.as_ref(), info)
    }
}
