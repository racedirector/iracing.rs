use std::{marker::PhantomData, path::Path, sync::Arc, time::Duration};

use super::{IbtProvider, Provider};
use crate::{IbtReader, Result, VariableSchema, runtime::Timer};

/// A [`Provider`] that replays `.ibt` frames using runtime-provided pacing.
pub struct ReplayProvider<TimerRuntime = crate::runtime::DefaultTimer> {
    ibt_provider: IbtProvider,

    /// Frame pacing interval.
    frame_interval: Duration,

    /// Playback speed multiplier (1.0 = normal, 2.0 = double speed).
    speed: f64,

    /// Native tick rate from IBT.
    tick_rate: f64,

    timer: PhantomData<TimerRuntime>,
}

impl<TimerRuntime> ReplayProvider<TimerRuntime> {
    /// Open a replay provider from an `.ibt` file path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let ibt_provider = IbtProvider::from_path(path)?;
        Ok(Self::with_provider(ibt_provider))
    }

    /// Build a replay provider from an already-open reader.
    pub fn with_reader(reader: IbtReader) -> Result<Self> {
        let ibt_provider = IbtProvider::with_reader(reader)?;
        Ok(Self::with_provider(ibt_provider))
    }

    /// Build a replay provider from an existing [`IbtProvider`].
    pub fn with_provider(provider: IbtProvider) -> Self {
        let tick_rate = provider.reader.tick_rate();
        let frame_interval = Duration::from_secs_f64(1.0 / tick_rate);

        Self {
            tick_rate,
            frame_interval,
            ibt_provider: provider,
            speed: 1.0,
            timer: PhantomData,
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

    /// Get current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_frame() as f64 / self.tick_rate
    }

    /// Total playback duration in seconds.
    pub fn duration(&self) -> f64 {
        self.total_frames() as f64 / self.tick_rate
    }

    fn pacing_interval(&self) -> Duration {
        Duration::from_secs_f64(self.frame_interval.as_secs_f64() / self.speed)
    }
}

#[async_trait::async_trait(?Send)]
impl<TimerRuntime> Provider for ReplayProvider<TimerRuntime>
where
    TimerRuntime: Timer,
{
    async fn next_frame(&mut self) -> Result<Option<crate::FramePacket>> {
        let next_frame = self.ibt_provider.next_frame().await?;

        if next_frame.is_some() {
            TimerRuntime::sleep(self.pacing_interval()).await;
        }

        Ok(next_frame)
    }

    async fn session_yaml(&mut self, version: u32) -> Result<Option<String>> {
        self.ibt_provider.session_yaml(version).await
    }
}
