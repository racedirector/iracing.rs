//! Telemetry spawns and manages telemetry processing tasks.

use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, provider::Provider, schema::SessionInfo};

/// A struct containing the telemetry communication channels and cancellation
/// token.
pub struct TelemetryChannels {
    /// Telemetry frame receiver.
    pub frames: watch::Receiver<Option<Arc<FramePacket>>>,
    /// Session info receiver.
    pub sessions: watch::Receiver<Option<Arc<SessionInfo>>>,

    /// Cancellation token for shutdown.
    pub cancel: CancellationToken,
}

/// `Telemetry` spawns and manages telemetry processing tasks.
///
/// Spawns a frame_read task that owns the `Provider` and detects session changes.
/// YAML parsing happens in a short-lived, spawned task to maintain <1ms frame latency.
pub struct Telemetry;

impl Telemetry {
    /// Spawn telemetry tasks for the given provider.
    ///
    /// Returns watch receivers for frames and sessions, and a cancellation token.
    pub fn spawn<P>(provider: P) -> TelemetryChannels
    where
        P: Provider,
    {
        Self::spawn_with_terminal_clear(provider, true)
    }

    /// Spawn telemetry tasks while retaining the last frame when the provider ends.
    ///
    /// Recorded connections use this so a short replay cannot replace its only
    /// frame with the end-of-stream sentinel before a subscriber is attached.
    pub(crate) fn spawn_preserving_last_frame<P>(provider: P) -> TelemetryChannels
    where
        P: Provider,
    {
        Self::spawn_with_terminal_clear(provider, false)
    }

    fn spawn_with_terminal_clear<P>(provider: P, clear_frame_on_end: bool) -> TelemetryChannels
    where
        P: Provider,
    {
        // Frame and session communication channels
        let (frame_tx, frame_rx) = watch::channel(None);
        let (session_tx, session_rx) = watch::channel(None);

        // Cancellation token for coordinated shutdown.
        let cancel = CancellationToken::new();

        // Cancellation token for the async task.
        let cancel_frame = cancel.clone();

        // Spawn the reader task; owns the provider.
        tokio::spawn(async move {
            Self::read_task(
                provider,
                frame_tx,
                session_tx,
                cancel_frame,
                clear_frame_on_end,
            )
            .await;
        });

        TelemetryChannels {
            frames: frame_rx,
            sessions: session_rx,
            cancel,
        }
    }

    // Reads frames and detects session changes.
    async fn read_task<P>(
        mut provider: P,
        frames: watch::Sender<Option<Arc<FramePacket>>>,
        session: watch::Sender<Option<Arc<SessionInfo>>>,
        cancel: CancellationToken,
        clear_frame_on_end: bool,
    ) where
        P: Provider,
    {
        tracing::info!("Frame reader task started");

        // Async task state
        let mut frame_count = 0u64;
        let mut error_count = 0u32;
        let mut last_session_version = None;

        const MAX_ERRORS: u32 = 10;

        loop {
            if cancel.is_cancelled() {
                tracing::info!("Frame read cancelled!");
                break;
            }

            let Some(result) = cancel.run_until_cancelled(provider.next_frame()).await else {
                tracing::info!("Frame read cancelled during read.");
                break;
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

                    // If the packet session version doesn't match, parse
                    // the latest session YAML from the provider and update
                    // observers.
                    if last_session_version != Some(version) {
                        tracing::debug!(
                            "Session version changed: {} -> {}",
                            last_session_version.unwrap_or(0),
                            version
                        );

                        match provider.session_yaml(version).await {
                            Ok(Some(yaml)) => {
                                tracing::debug!(
                                    "Fetched session YAML ({} bytes) for v{}",
                                    yaml.len(),
                                    version
                                );

                                // Clone the session channel for the parsing task.
                                let session_clone = session.clone();

                                // Spawn task to parse YAML without blocking the frame reader.
                                tokio::spawn(async move {
                                    match SessionInfo::parse(&yaml) {
                                        Ok(session) => {
                                            tracing::debug!("Parsed session info");
                                            let _ = session_clone.send(Some(Arc::new(session)));
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to parse session YAML: {}", e);
                                        }
                                    }
                                });
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
                    if frames.send(Some(Arc::new(packet))).is_err() {
                        tracing::debug!("Frame receiver dropped. Shutting down.");
                        break;
                    }
                }
                Ok(None) => {
                    tracing::info!("Provider stream ended after {} frames", frame_count);
                    // Update consumers that stream ended.
                    if clear_frame_on_end {
                        let _ = frames.send(None);
                    }
                    let _ = session.send(None);
                    break;
                }
                Err(e) => {
                    // Provider error
                    error_count += 1;
                    tracing::error!("Provider error ({}/{}): {}", error_count, MAX_ERRORS, e);

                    if error_count >= MAX_ERRORS {
                        tracing::error!("Too many provider errors, shutting down!");
                        // Update consumers that stream ended.
                        if clear_frame_on_end {
                            let _ = frames.send(None);
                        }
                        let _ = session.send(None);
                        break;
                    }

                    // Exponential backoff before next loop.
                    let backoff = std::time::Duration::from_millis(50 * (1 << error_count.min(5)));
                    tokio::time::sleep(backoff).await;
                }
            }
        }

        // When the loop breaks, log the number of processed frames.
        tracing::info!("Frame reader task ended (processed {} frames)", frame_count);
    }
}
