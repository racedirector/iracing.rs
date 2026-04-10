//! Dynamic key-value adapter over a telemetry frame.
//!
//! This adapter provides ergonomic, by-name lookups for variables without
//! requiring a bespoke typed struct. It is intended for exploration, tooling,
//! and diagnostics. For hot paths, prefer typed adapters that implement
//! [`FrameAdapter`] so validation happens once and frame extraction stays cheap.

use crate::{
    FramePacket, Result, VarData, VariableInfo, VariableSchema,
    adapters::{AdapterValidation, FrameAdapter},
};
use std::sync::Arc;

/// A self-contained view over a single telemetry frame supporting by-name lookups.
#[derive(Debug, Clone)]
pub struct DynamicFrame {
    data: Arc<[u8]>,
    tick_count: u32,
    schema: Arc<VariableSchema>,
}

impl DynamicFrame {
    /// Returns variable metadata if present.
    pub fn variable_info(&self, name: &str) -> Option<&VariableInfo> {
        self.schema.variables.get(name)
    }

    /// Generic typed lookup by variable name.
    /// Returns None if the variable is missing or type conversion fails.
    pub fn get<T: VarData>(&self, name: &str) -> Option<T> {
        let info = self.variable_info(name)?;
        T::from_bytes(self.data.as_ref(), info).ok()
    }

    /// Look up a variable as `f32`, or `None` if missing or the wrong type.
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.get(name)
    }

    /// Look up a variable as `i32`, or `None` if missing or the wrong type.
    pub fn i32(&self, name: &str) -> Option<i32> {
        self.get(name)
    }

    /// Look up a variable as `u32`, or `None` if missing or the wrong type.
    pub fn u32(&self, name: &str) -> Option<u32> {
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
}

impl FrameAdapter for DynamicFrame {
    fn validate_schema(_schema: &VariableSchema) -> Result<AdapterValidation> {
        // No pre-validation or extraction plan needed for dynamic lookups
        Ok(AdapterValidation::new(Vec::new()))
    }

    fn adapt(packet: &FramePacket, _validation: &AdapterValidation) -> Self {
        Self {
            data: Arc::clone(&packet.data),
            tick_count: packet.tick,
            schema: Arc::clone(&packet.schema),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VariableInfo, VariableSchema, types::VariableType};
    use std::collections::HashMap;

    #[test]
    fn dynamic_frame_basic_lookup() {
        // Build minimal schema
        let mut vars = HashMap::new();
        vars.insert(
            "RPM".to_string(),
            VariableInfo {
                name: "RPM".into(),
                data_type: VariableType::Int32,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: "rev/min".into(),
                description: "Engine RPM".into(),
            },
        );
        vars.insert(
            "Speed".to_string(),
            VariableInfo {
                name: "Speed".into(),
                data_type: VariableType::Float32,
                offset: 4,
                count: 1,
                count_as_time: false,
                units: "m/s".into(),
                description: "Vehicle speed".into(),
            },
        );
        vars.insert(
            "CarIdxLapDistPct".to_string(),
            VariableInfo {
                name: "CarIdxLapDistPct".into(),
                data_type: VariableType::Float32,
                offset: 8,
                count: 4,
                count_as_time: false,
                units: "%".into(),
                description: "Per-car lap distance percentage".into(),
            },
        );
        let schema = VariableSchema {
            variables: vars,
            frame_size: 24,
        };

        // Build frame bytes (Int32 + Float32 + four Float32 array elements)
        let mut data = vec![0u8; 24];
        data[0..4].copy_from_slice(&1234i32.to_le_bytes());
        data[4..8].copy_from_slice(&42.5f32.to_le_bytes());
        let lap_dist = [0.10f32, 0.20, 0.30, 0.40];
        for (idx, value) in lap_dist.iter().enumerate() {
            let start = 8 + idx * 4;
            data[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }

        let packet = FramePacket::new(data, 10, 0, Arc::new(schema));
        let df = DynamicFrame::adapt(&packet, &AdapterValidation::new(vec![]));

        assert_eq!(df.i32("RPM"), Some(1234));
        assert!(df.f32("Speed").unwrap() - 42.5 < 1e-5);
        let lap_dist_values: Vec<f32> = df.get("CarIdxLapDistPct").unwrap();
        assert_eq!(lap_dist_values, lap_dist);
        assert_eq!(df.u32("Missing"), None);
    }
}
