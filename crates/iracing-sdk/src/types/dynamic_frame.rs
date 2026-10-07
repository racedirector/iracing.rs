//! Dynamic key-value adapter over a telemetry frame.
//!
//! This adapter provides ergonomic, by-name lookups for variables without
//! requiring a bespoke typed struct. It is intended for exploration, tooling,
//! and diagnostics. For hot paths, prefer typed adapters that implement
//! [`FrameAdapter`] so validation happens once and frame extraction stays cheap.

use crate::{
    BitField, FieldLayout, FramePacket, LayoutProvider, Result, TelemetryLayout, TelemetryValue,
    VarData,
    adapters::{AdapterValidation, FrameAdapter},
    types::telemetry_value::TelemetryValueProvider,
};
use std::sync::Arc;

/// A self-contained view over a single telemetry frame supporting by-name lookups.
#[derive(Debug, Clone)]
pub struct DynamicFrame {
    data: Arc<[u8]>,
    tick_count: u32,
    layout: Arc<TelemetryLayout>,
}

impl DynamicFrame {
    /// Generic typed lookup by variable name.
    /// Returns `None` if the variable is missing or decoding fails, including
    /// incompatible storage types, scalar/array shapes, or invalid enum values.
    pub fn get<T: VarData>(&self, name: &str) -> Option<T> {
        let info = self.field_named(name)?;
        T::decode_field(self.data.as_ref(), info).ok()
    }

    /// Look up a variable as `f32`, or `None` if missing or the wrong type.
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.get(name)
    }

    /// Look up a variable as `i32`, or `None` if missing or the wrong type.
    pub fn i32(&self, name: &str) -> Option<i32> {
        self.get(name)
    }

    /// Look up an SDK bitfield, or `None` if missing or the wrong type.
    pub fn bitfield(&self, name: &str) -> Option<BitField> {
        self.get(name)
    }

    /// Look up a variable as `f64`, or `None` if missing or the wrong type.
    pub fn f64(&self, name: &str) -> Option<f64> {
        self.get(name)
    }

    /// Look up a variable as `bool`, or `None` if missing or the wrong type.
    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get(name)
    }

    /// Monotonic frame counter for this frame.
    pub fn tick_count(&self) -> u32 {
        self.tick_count
    }

    /// Retrieves the variable from the frame by name, or `Ok(None)` if absent.
    pub fn value(&self, name: &str) -> Result<Option<TelemetryValue>> {
        let Some(info) = self.field_named(name) else {
            return Ok(None);
        };

        self.telemetry_value(info).map(Some)
    }
}

impl LayoutProvider for DynamicFrame {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
}

impl TelemetryValueProvider for DynamicFrame {
    fn telemetry_value(&self, info: &FieldLayout) -> Result<TelemetryValue> {
        TelemetryValue::decode_field(self.data.as_ref(), info)
    }
}

impl FrameAdapter for DynamicFrame {
    fn validate_layout(layout: &Arc<TelemetryLayout>) -> Result<AdapterValidation> {
        // No pre-validation or extraction plan needed for dynamic lookups
        Ok(AdapterValidation::new(Arc::clone(layout), Vec::new()))
    }

    /// Shares the packet's bytes and layout for subsequent dynamic lookups.
    ///
    /// # Panics
    /// Panics if `validation` retains a different layout allocation.
    fn adapt(packet: &FramePacket, validation: &AdapterValidation) -> Self {
        validation
            .ensure_packet(packet)
            .expect("adapter layout mismatch");
        Self {
            data: Arc::clone(packet.data()),
            tick_count: packet.tick,
            layout: Arc::clone(packet.layout()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irsdk::VariableType;
    use std::collections::HashMap;

    #[test]
    fn dynamic_frame_basic_lookup() {
        // Build minimal layout
        let mut vars = HashMap::new();
        vars.insert(
            "RPM".to_string(),
            crate::test_utils::field(
                "RPM".into(),
                VariableType::Integer,
                0,
                1,
                false,
                "rev/min".into(),
                "Engine RPM".into(),
            ),
        );
        vars.insert(
            "Speed".to_string(),
            crate::test_utils::field(
                "Speed".into(),
                VariableType::Float,
                4,
                1,
                false,
                "m/s".into(),
                "Vehicle speed".into(),
            ),
        );
        vars.insert(
            "CarIdxLapDistPct".to_string(),
            crate::test_utils::field(
                "CarIdxLapDistPct".into(),
                VariableType::Float,
                8,
                4,
                false,
                "%".into(),
                "Per-car lap distance percentage".into(),
            ),
        );
        let layout = crate::test_utils::layout((vars).into_values(), 24).unwrap();

        // Build frame bytes (Int32 + Float32 + four Float32 array elements)
        let mut data = vec![0u8; 24];
        data[0..4].copy_from_slice(&1234i32.to_le_bytes());
        data[4..8].copy_from_slice(&42.5f32.to_le_bytes());
        let lap_dist = [0.10f32, 0.20, 0.30, 0.40];
        for (idx, value) in lap_dist.iter().enumerate() {
            let start = 8 + idx * 4;
            data[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }

        let packet = FramePacket::new(data, 10, 0, Arc::new(layout)).unwrap();
        let df = DynamicFrame::adapt(
            &packet,
            &DynamicFrame::validate_layout(packet.layout()).unwrap(),
        );

        assert!(Arc::ptr_eq(df.layout(), packet.layout()));
        assert!(df.has_field("RPM"));
        assert!(!df.has_field("Missing"));

        let rpm_info = df.field_named("RPM").unwrap();
        assert_eq!(
            df.telemetry_value(rpm_info).unwrap(),
            TelemetryValue::Int32(1234)
        );
        assert_eq!(df.value("RPM").unwrap(), Some(TelemetryValue::Int32(1234)));
        assert_eq!(df.value("Missing").unwrap(), None);

        assert_eq!(df.i32("RPM"), Some(1234));
        assert!(df.f32("Speed").unwrap() - 42.5 < 1e-5);
        let lap_dist_values: Vec<f32> = df.get("CarIdxLapDistPct").unwrap();
        assert_eq!(lap_dist_values, lap_dist);
        assert_eq!(df.bitfield("Missing"), None);
    }
}
