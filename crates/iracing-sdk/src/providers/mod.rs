use crate::{FramePacket, Result};

/// Source of telemetry frames and session data.
///
/// Drive a provider in a loop: call [`Provider::next_frame`] until it returns
/// `Ok(None)` (end of file) or an error. When `session_version` on a
/// [`FramePacket`] changes, call [`Provider::session_yaml`] with the new
/// version to retrieve updated session info YAML.

#[async_trait::async_trait(?Send)]
pub trait Provider {
    /// Return the next telemetry frame, or `Ok(None)` when the source is exhausted.
    async fn next_frame(&mut self) -> Result<Option<FramePacket>>;

    /// Return the session info YAML for `version`, or `Ok(None)` if unchanged.
    async fn session_yaml(&mut self, version: u32) -> Result<Option<String>>;

    /// Get the native tick rate in Hz
    ///
    /// This is the source frequency (e.g., 60Hz for live, varies for replays)
    fn tick_rate(&self) -> f64;
}

/// IBT file provider.
mod ibt;

/// Replay IBT provider.
mod replay;

/// Live shared-memory provider (Windows only).
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
mod live;

pub use ibt::IbtProvider;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use live::DefaultLiveProvider;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use live::LiveProvider;
pub use replay::ReplayProvider;
