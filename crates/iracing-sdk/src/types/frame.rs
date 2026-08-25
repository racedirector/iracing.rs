//! Frame packet types for stream-based architecture

use std::sync::Arc;

use crate::{SchemaProvider, TelemetryValue, TelemetryValueProvider, VariableInfo, VariableSchema};

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
    /// Retrieves the variable from the frame by name.
    pub fn value(&self, name: &str) -> crate::Result<Option<TelemetryValue>> {
        let Some(info) = self.variable(name) else {
            return Ok(None);
        };

        self.telemetry_value(info).map(Some)
    }
}

impl SchemaProvider for FramePacket {
    fn schema(&self) -> &VariableSchema {
        &self.schema
    }
}

impl TelemetryValueProvider for FramePacket {
    fn telemetry_value(&self, info: &VariableInfo) -> crate::Result<TelemetryValue> {
        TelemetryValue::decode(self.data.as_ref(), info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableType;
    use std::collections::HashMap;

    #[test]
    fn frame_packet_provides_schema_and_telemetry_values() {
        let rpm_info = VariableInfo {
            name: "RPM".into(),
            data_type: VariableType::Int32,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: "rev/min".into(),
            description: "Engine RPM".into(),
        };
        let schema = Arc::new(VariableSchema {
            variables: HashMap::from([("RPM".to_string(), rpm_info)]).into(),
            frame_size: 4,
        });
        let packet = FramePacket::new(1234i32.to_le_bytes().to_vec(), 10, 2, Arc::clone(&schema));

        assert!(std::ptr::eq(packet.schema(), schema.as_ref()));
        assert!(packet.has_variable("RPM"));
        assert!(!packet.has_variable("Missing"));

        let info = packet.variable("RPM").unwrap();
        assert_eq!(
            packet.telemetry_value(info).unwrap(),
            TelemetryValue::Int32(1234)
        );
        assert_eq!(
            packet.value("RPM").unwrap(),
            Some(TelemetryValue::Int32(1234))
        );
        assert_eq!(packet.value("Missing").unwrap(), None);
    }
}
