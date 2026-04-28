use std::{path::Path, sync::Arc};
use tokio::time::{Duration, Interval, interval};

use super::{IbtProvider, Provider};
use crate::{IbtReader, Result, VariableSchema};

/// A [`Provider`] that streams telemetry frames from an iRacing `ibt` file the same way as it was recorded.
pub struct ReplayProvider {
    ibt_provider: IbtProvider,

    /// Frame pacing interval
    interval: Interval,

    /// Playback speed multiplier (1.0 = normal, 2.0 = double speed)
    speed: f64,

    /// Native tick rate from IBT
    tick_rate: f64,
}

impl ReplayProvider {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let ibt_provider = IbtProvider::from_path(path)?;
        Ok(Self::with_provider(ibt_provider))
    }

    pub fn with_reader(reader: IbtReader) -> Result<Self> {
        let ibt_provider = IbtProvider::with_reader(reader)?;
        Ok(Self::with_provider(ibt_provider))
    }

    pub fn with_provider(provider: IbtProvider) -> Self {
        // Get metadata
        let total_frames = provider.reader.total_frames();
        let tick_rate = provider.reader.tick_rate();

        // Calculate frame interval for pacing
        let frame_interval = Duration::from_secs_f64(1.0 / tick_rate);
        let interval = interval(frame_interval);

        Self {
            tick_rate,
            interval,
            ibt_provider: provider,
            speed: 1.0,
        }
    }

    /// Returns a shared reference to the telemetry variable schema.
    pub fn schema(&self) -> Arc<VariableSchema> {
        self.ibt_provider.schema()
    }

    /// Returns the index of the next frame that will be read (0-based).
    pub fn current_frame(&self) -> usize {
        self.ibt_provider.current_frame()
    }

    /// Returns the total number of telemetry frames in the file.
    pub fn total_frames(&self) -> usize {
        self.ibt_provider.total_frames()
    }

    /// Get current playback time in seconds
    pub fn current_time(&self) -> f64 {
        self.current_frame() as f64 / self.tick_rate
    }

    /// Native tick rate from IBT
    pub fn duration(&self) -> f64 {
        self.total_frames() as f64 / self.tick_rate
    }
}

impl Provider for ReplayProvider {
    fn next_frame(&mut self) -> Result<Option<crate::FramePacket>> {
        let next_frame = self.ibt_provider.next_frame()?;

        // Wait for next frame timing (pacing)
        // self.interval.tick().await;

        Ok(next_frame)
    }

    fn session_yaml(&mut self, version: u32) -> Result<Option<String>> {
        self.ibt_provider.session_yaml(version)
    }
}
