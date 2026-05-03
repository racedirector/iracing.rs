//! TelemetryEmitter spawns and manages telemetry processing tasks.

use std::sync::Arc;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, Provider, SessionInfo};

/// Result of spawning telemetry tasks
pub struct TelemetryEmitterChannels {
    /// Receiver for telemetry frames
    pub frames: watch::Receiver<Option<Arc<FramePacket>>>,
    /// Receiver for session info updates
    pub sessions: watch::Receiver<Option<Arc<SessionInfo>>>,
    /// Cancellation token for graceful shutdown.
    pub cancel: CancellationToken,
}

/// TelemetryEmitter spawns and manages telemetry processing tasks.
///
/// Spawns a frame reader that owns the provider, and detects session changes.
pub struct TelemetryEmitter;

impl TelemetryEmitter {
    /// Spawn telemetry processing on the native Tokio runtime.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<P>(provider: P) -> TelemetryEmitterChannels
    where
        P: Provider + Send + 'static,
    {
        let (channels, frame_tx, session_tx) = Self::channels();
        let cancel_frame = channels.cancel.clone();

        tokio::spawn(async move {
            Self::frame_output_task(provider, frame_tx, session_tx, cancel_frame).await;
        });

        channels
    }

    /// Spawn telemetry processing on the current wasm/browser task queue.
    #[cfg(target_arch = "wasm32")]
    pub fn spawn_local<P>(provider: P) -> TelemetryEmitterChannels
    where
        P: Provider + 'static,
    {
        let (channels, frame_tx, session_tx) = Self::channels();
        let cancel_frame = channels.cancel.clone();

        wasm_bindgen_futures::spawn_local(async move {
            Self::frame_output_task(provider, frame_tx, session_tx, cancel_frame).await;
        });

        channels
    }

    fn channels() -> (
        TelemetryEmitterChannels,
        watch::Sender<Option<Arc<FramePacket>>>,
        watch::Sender<Option<Arc<SessionInfo>>>,
    ) {
        let (frame_tx, frame_rx) = watch::channel(None);
        let (session_tx, session_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        (
            TelemetryEmitterChannels {
                frames: frame_rx,
                sessions: session_rx,
                cancel,
            },
            frame_tx,
            session_tx,
        )
    }

    /// Output task - reads frames and session changes and outputs them
    async fn frame_output_task<P>(
        mut provider: P,
        frame_tx: watch::Sender<Option<Arc<FramePacket>>>,
        session_tx: watch::Sender<Option<Arc<SessionInfo>>>,
        cancel: CancellationToken,
    ) where
        P: Provider,
    {
        tracing::info!("Frame output task started");

        // Local state tracking
        let mut frame_count = 0u64;
        let mut error_count = 0u32;
        let mut last_session_version: Option<u32> = None;
        const MAX_ERRORS: u32 = 10;

        loop {
            if cancel.is_cancelled() {
                tracing::info!("Frame output cancelled");
                break;
            }

            let result = tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Frame output cancelled during read");
                    break;
                }
                result = provider.next_frame() => result,
            };

            match result {
                Ok(Some(packet)) => {
                    frame_count += 1;
                    error_count = 0;
                    let version = packet.session_version;

                    tracing::trace!(
                        "Frame {}: tick={}, session_version={}",
                        frame_count,
                        packet.tick,
                        version
                    );

                    if last_session_version != Some(version) {
                        tracing::debug!(
                            "Session version changed: {} -> {}",
                            last_session_version.unwrap_or(0),
                            version
                        );

                        match provider.session_yaml(version).await {
                            Ok(Some(yaml)) => {
                                tracing::debug!(
                                    "Fetched session YAML ({} bytes) for version {}",
                                    yaml.len(),
                                    version
                                );

                                match SessionInfo::parse(&yaml) {
                                    Ok(session) => {
                                        tracing::debug!("Session parsed");
                                        let _ = session_tx.send(Some(Arc::new(session)));
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to parse session YAML: {}", e);
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::debug!("No session YAML for version {}", version);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to get session YAML: {}", e);
                            }
                        }

                        last_session_version = Some(version);
                    }

                    // Always send the frame
                    if frame_tx.send(Some(Arc::new(packet))).is_err() {
                        tracing::debug!("Frame receiver dropped, shutting down");
                        break;
                    }
                }
                Ok(None) => {
                    tracing::info!("Provider stream ended after {} frames", frame_count);
                    // Send None to indicate end of stream
                    let _ = frame_tx.send(None);
                    let _ = session_tx.send(None);
                    break;
                }
                Err(e) => {
                    error_count += 1;
                    tracing::error!("Provider error ({}/{}): {}", error_count, MAX_ERRORS, e);

                    if error_count >= MAX_ERRORS {
                        tracing::error!("Too many provider errors, shutting down");
                        // Send None to indicate end of stream
                        let _ = frame_tx.send(None);
                        let _ = session_tx.send(None);
                        break;
                    }

                    // Exponential backoff: 50ms, 100ms, 200ms, ...
                    let backoff = std::time::Duration::from_millis(50 * (1 << error_count.min(5)));
                    sleep(backoff).await;
                }
            }
        }

        tracing::info!("Frame reader task ended (processed {} frames)", frame_count);
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: std::time::Duration) {
    let millis = duration.as_millis();
    let millis = if millis == 0 && !duration.is_zero() {
        1
    } else {
        millis.min(u32::MAX as u128) as u32
    };

    gloo_timers::future::TimeoutFuture::new(millis).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Arc, time::Duration};

    use tokio::{sync::oneshot, time::timeout};

    use crate::{Result, VariableSchema};

    enum MockNext {
        Frame(FramePacket),
        Pending(oneshot::Receiver<()>),
    }

    struct MockProvider {
        next: VecDeque<MockNext>,
        session_yaml: Option<String>,
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            match self.next.pop_front() {
                Some(MockNext::Frame(packet)) => Ok(Some(packet)),
                Some(MockNext::Pending(receiver)) => {
                    let _ = receiver.await;
                    Ok(None)
                }
                None => Ok(None),
            }
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(self.session_yaml.clone())
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    #[tokio::test]
    async fn emits_frame_updates() {
        let (_tx, rx) = oneshot::channel();
        let provider = MockProvider {
            next: VecDeque::from([MockNext::Frame(make_frame(42, 1)), MockNext::Pending(rx)]),
            session_yaml: None,
        };
        let mut channels = TelemetryEmitter::spawn(provider);

        channels.frames.changed().await.unwrap();
        let frame = channels.frames.borrow().as_ref().unwrap().clone();

        assert_eq!(frame.tick, 42);
        channels.cancel.cancel();
    }

    #[tokio::test]
    async fn emits_session_updates_before_frame_for_new_version() {
        let (_tx, rx) = oneshot::channel();
        let provider = MockProvider {
            next: VecDeque::from([MockNext::Frame(make_frame(42, 7)), MockNext::Pending(rx)]),
            session_yaml: Some(
                r#"
WeekendInfo: {}
SessionInfo: {}
"#
                .to_string(),
            ),
        };
        let mut channels = TelemetryEmitter::spawn(provider);

        channels.sessions.changed().await.unwrap();
        assert!(channels.sessions.borrow().is_some());

        channels.frames.changed().await.unwrap();
        assert_eq!(
            channels.frames.borrow().as_ref().unwrap().session_version,
            7
        );
        channels.cancel.cancel();
    }

    #[tokio::test]
    async fn sends_none_when_stream_ends() {
        let provider = MockProvider {
            next: VecDeque::new(),
            session_yaml: None,
        };
        let mut channels = TelemetryEmitter::spawn(provider);

        channels.frames.changed().await.unwrap();

        assert!(channels.frames.borrow().is_none());
    }

    #[tokio::test]
    async fn cancellation_closes_frame_channel() {
        let (_tx, rx) = oneshot::channel();
        let provider = MockProvider {
            next: VecDeque::from([MockNext::Pending(rx)]),
            session_yaml: None,
        };
        let mut channels = TelemetryEmitter::spawn(provider);

        channels.cancel.cancel();

        assert!(
            timeout(Duration::from_secs(1), channels.frames.changed())
                .await
                .unwrap()
                .is_err()
        );
    }

    fn make_frame(tick: u32, session_version: u32) -> FramePacket {
        let schema = VariableSchema::new(std::collections::HashMap::new(), 1).unwrap();
        FramePacket::new(vec![0], tick, session_version, Arc::new(schema))
    }
}
