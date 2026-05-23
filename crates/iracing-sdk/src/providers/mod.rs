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

/// A Windows-only provider extension whose async operations are safe to await
/// from `Send` futures.
///
/// The base [`Provider`] trait intentionally uses `?Send` so replay providers
/// can remain usable in WASM contexts. Live Windows telemetry can satisfy the
/// stronger bound, so adapters that run under multi-threaded async services can
/// opt into this trait without changing the cross-platform provider contract.
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
#[async_trait::async_trait]
pub trait SendProvider: Send {
    /// Return the next telemetry frame, or `Ok(None)` when the source is exhausted.
    async fn next_frame_send(&mut self) -> Result<Option<FramePacket>>;

    /// Return the session info YAML for `version`, or `Ok(None)` if unchanged.
    async fn session_yaml_send(&mut self, version: u32) -> Result<Option<String>>;
}

/// IBT replay file provider.
mod ibt;

/// Live shared-memory provider (Windows only).
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
mod live;

pub use ibt::IbtProvider;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use live::LiveProvider;
