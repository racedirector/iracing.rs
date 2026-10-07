//! Owned telemetry frames with an exact runtime layout contract.
use crate::{
    FieldLayout, IRacingSDKError, LayoutProvider, Result, TelemetryLayout, TelemetryValue,
    TelemetryValueProvider,
};
use std::sync::Arc;

/// One complete frame associated with the layout that describes its bytes.
#[derive(Debug, Clone)]
pub struct FramePacket {
    data: Arc<[u8]>,
    layout: Arc<TelemetryLayout>,
    /// Source tick counter.
    pub tick: u32,
    /// Source session-information revision.
    pub session_version: u32,
}
impl FramePacket {
    /// Constructs exactly one frame without truncation.
    ///
    /// # Errors
    /// Rejects both short and oversized buffers relative to the layout.
    pub fn new(
        data: impl Into<Arc<[u8]>>,
        tick: u32,
        session_version: u32,
        layout: Arc<TelemetryLayout>,
    ) -> Result<Self> {
        let data = data.into();
        if data.len() != layout.frame_size() {
            return Err(IRacingSDKError::WireSize {
                expected: layout.frame_size(),
                actual: data.len(),
            });
        }
        Ok(Self {
            data,
            layout,
            tick,
            session_version,
        })
    }
    /// Borrows the immutable complete frame bytes.
    pub fn data(&self) -> &Arc<[u8]> {
        &self.data
    }
    /// Returns the originating shared layout.
    pub fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
    /// Decodes a published name, returning `None` for an absent field.
    pub fn value(&self, name: &str) -> Result<Option<TelemetryValue>> {
        self.layout
            .field_by_name(name)
            .map(|(_, field)| self.telemetry_value(field))
            .transpose()
    }
}
impl LayoutProvider for FramePacket {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
}
impl TelemetryValueProvider for FramePacket {
    fn telemetry_value(&self, field: &FieldLayout) -> Result<TelemetryValue> {
        TelemetryValue::decode_field(&self.data, field)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        VariableHeaders,
        irsdk::{VariableHeader, VariableType},
    };
    #[test]
    fn accepts_only_exact_frames_and_reuses_layout() {
        let headers = VariableHeaders::new(&[VariableHeader::new(
            VariableType::Float,
            0,
            1,
            false,
            "Speed",
            "",
            "m/s",
        )
        .unwrap()]);
        let layout = Arc::new(TelemetryLayout::try_from_headers(&headers, 4).unwrap());
        for actual in [3, 5] {
            assert!(
                matches!(FramePacket::new(vec![0;actual],0,0,layout.clone()), Err(IRacingSDKError::WireSize { expected:4, actual:n }) if n==actual)
            );
        }
        let packet = FramePacket::new(42f32.to_le_bytes().to_vec(), 1, 2, layout.clone()).unwrap();
        assert!(Arc::ptr_eq(packet.layout(), &layout));
        assert_eq!(
            packet.value("Speed").unwrap(),
            Some(TelemetryValue::Float32(42.0))
        );
        assert_eq!(packet.value("Missing").unwrap(), None);
    }
}
