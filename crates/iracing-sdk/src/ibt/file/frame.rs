//! Recorded frame ownership and the packet compatibility boundary.

use std::sync::Arc;

use crate::{
    FieldLayout, FramePacket, IRacingSDKError, LayoutProvider, Result, TelemetryLayout,
    TelemetryValue, TelemetryValueProvider,
};

/// One recorded frame bound to the exact telemetry layout of its recording.
///
/// The zero-based `usize` index identifies a physical record, independently of
/// live SDK ticks. Bytes and layout remain available after the file is dropped;
/// cloning a frame shares both allocations.
#[derive(Debug, Clone)]
pub struct IbtFrame {
    index: usize,
    bytes: Arc<[u8]>,
    layout: Arc<TelemetryLayout>,
    session_info_update: i32,
}

impl IbtFrame {
    pub(super) fn new(
        index: usize,
        bytes: Arc<[u8]>,
        layout: Arc<TelemetryLayout>,
        session_info_update: i32,
    ) -> Self {
        Self {
            index,
            bytes,
            layout,
            session_info_update,
        }
    }

    /// Returns the zero-based physical record index within the recording.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Borrows the immutable complete frame bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes this frame and returns its shared owned bytes without copying.
    pub fn into_bytes(self) -> Arc<[u8]> {
        self.bytes
    }

    /// Returns the exact shared layout established by the originating file.
    pub fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }

    /// Decodes a published field, returning `None` for an absent name.
    pub fn value(&self, name: &str) -> Result<Option<TelemetryValue>> {
        self.layout
            .field_by_name(name)
            .map(|(_, field)| self.telemetry_value(field))
            .transpose()
    }

    /// Converts this recorded frame to the SDK's streaming packet format.
    ///
    /// This is the recorded-data compatibility bridge. Its synthetic packet
    /// tick is the record index; neither ticks nor the packet's `u32` range
    /// define native recorded identity. Bytes and layout are transferred without
    /// copying or rebuilding metadata.
    ///
    /// # Errors
    /// Rejects indices outside the packet's `u32` range and negative recorded
    /// session revisions rather than wrapping them. Packet construction also
    /// validates exact frame size against the retained layout.
    pub fn into_packet(self) -> Result<FramePacket> {
        let tick = u32::try_from(self.index).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtFrame::into_packet",
                format!("Frame index {} exceeds the u32 tick range", self.index),
            )
        })?;
        let session_version = u32::try_from(self.session_info_update).map_err(|_| {
            IRacingSDKError::parse_error(
                "IbtFrame::into_packet",
                format!(
                    "Recorded session revision {} is outside the u32 packet range",
                    self.session_info_update
                ),
            )
        })?;
        FramePacket::new(self.bytes, tick, session_version, self.layout)
    }
}

impl LayoutProvider for IbtFrame {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
}

impl TelemetryValueProvider for IbtFrame {
    fn telemetry_value(&self, field: &FieldLayout) -> Result<TelemetryValue> {
        TelemetryValue::decode_field(&self.bytes, field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ibt::IbtFile, test_utils::require_smallest_ibt_fixture};

    #[test]
    fn packet_bridge_checks_revision_and_frame_size() -> anyhow::Result<()> {
        let file = IbtFile::open(require_smallest_ibt_fixture()?)?;
        let mut frame = file.frame(0)?;
        frame.session_info_update = -1;
        assert!(
            frame
                .into_packet()
                .unwrap_err()
                .to_string()
                .contains("session revision")
        );

        let mut frame = file.frame(0)?;
        frame.bytes = Arc::from(&frame.bytes[..frame.bytes.len() - 1]);
        assert!(matches!(
            frame.into_packet(),
            Err(IRacingSDKError::WireSize { .. })
        ));
        Ok(())
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn native_record_identity_is_wider_than_packet_tick() -> anyhow::Result<()> {
        let file = IbtFile::open(require_smallest_ibt_fixture()?)?;
        let mut frame = file.frame(0)?;
        frame.index = u32::MAX as usize;
        assert_eq!(frame.into_packet()?.tick, u32::MAX);
        let mut frame = file.frame(0)?;
        frame.index = u32::MAX as usize + 1;
        assert_eq!(frame.index(), u32::MAX as usize + 1);
        assert!(
            frame
                .into_packet()
                .unwrap_err()
                .to_string()
                .contains("u32 tick range")
        );
        Ok(())
    }
}
