//! Provider trait for data sources

use super::types::FramePacket;
use crate::Result;

/// Trait for telemetry data sources.
///
/// Providers abstract over different data sources and handle their own timing internally.
#[async_trait::async_trait]
pub trait Provider: Send + 'static {
    /// Return the next telemetry frame, or `Ok(None)` when the source is exhausted.
    async fn next_frame(&mut self) -> Result<Option<FramePacket>>;

    /// Return the session info YAML for `version`, or `Ok(None)` if unchanged.
    async fn session_yaml(&mut self, version: u32) -> Result<Option<String>>;

    /// Get the native tick rate in Hz
    ///
    /// This is the source frequency (e.g., 60Hz for live, varies for replays)
    fn tick_rate(&self) -> f64;
}
