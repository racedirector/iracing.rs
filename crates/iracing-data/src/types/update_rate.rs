//! Update rate control for telemetry streams

use serde::{Deserialize, Serialize};

/// Update rate for telemetry streams
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UpdateRate {
    /// Full speed from source (typically 60Hz)
    Native,

    /// Throttled to maximum Hz
    /// If the requested rate exceeds source rate, Native is used
    Max(u32),
}

impl UpdateRate {
    /// Normalize rate against source frequency
    /// Returns effective rate to use
    pub fn normalize(self, source_hz: f64) -> Self {
        match self {
            UpdateRate::Native => UpdateRate::Native,
            UpdateRate::Max(hz) if hz as f64 >= source_hz => UpdateRate::Native,
            UpdateRate::Max(hz) => UpdateRate::Max(hz),
        }
    }

    /// Check if throttling is needed
    pub fn needs_throttle(self, source_hz: f64) -> bool {
        match self.normalize(source_hz) {
            UpdateRate::Native => false,
            UpdateRate::Max(_) => true,
        }
    }

    /// Get throttle interval if needed
    pub fn throttle_interval(self, source_hz: f64) -> Option<std::time::Duration> {
        match self.normalize(source_hz) {
            UpdateRate::Native => None,
            UpdateRate::Max(hz) => Some(std::time::Duration::from_secs_f64(1.0 / hz as f64)),
        }
    }
}
