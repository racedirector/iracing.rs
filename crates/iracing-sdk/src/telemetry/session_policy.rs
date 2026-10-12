use std::sync::Arc;

use async_trait::async_trait;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, provider::Provider, schema::SessionInfo};

/// Controls how session information is discovered and published for a telemetry source.
///
/// Live telemetry publishes a new session whenever its session version changes.
/// Policies separate initialization, per-frame observation, and finalization
/// from the shared telemetry read loop.
///
/// The telemetry task calls the methods in this order:
///
/// 1. [`SessionPolicy::initialize`] once, before requesting or reading frames.
/// 2. [`SessionPolicy::observe`] for every successfully read frame.
/// 3. [`SessionPolicy::end`] exactly once whenever the telemetry task stops,
///    including provider EOF, terminal errors, and cancellation.
#[async_trait]
pub(crate) trait SessionPolicy<P: Provider>: Send {
    /// Perform source-specific session initialization before frame processing.
    ///
    /// The default implementation does nothing, which is appropriate for live
    /// telemetry because its session is discovered from frame version changes.
    ///
    /// Implementations should return `true` when frame processing may continue.
    /// Returning `false` tells the telemetry task to stop, normally because
    /// initialization was cancelled. Initialization failures that should not
    /// prevent frame playback may be recorded by the policy and still return
    /// `true`.
    async fn initialize(&mut self, _provider: &mut P, _cancel: &CancellationToken) -> bool {
        true
    }

    /// Observe one frame and perform any session work it makes necessary.
    ///
    /// This method runs after the provider produces a frame but before that
    /// frame is handed to the delivery policy. A live policy uses the frame's
    /// session version to decide whether to fetch updated YAML.
    ///
    /// Implementations should return `true` to allow the frame to be delivered.
    /// Returning `false` stops the telemetry task and prevents delivery of the
    /// current frame.
    async fn observe(
        &mut self,
        provider: &mut P,
        frame: &FramePacket,
        cancel: &CancellationToken,
    ) -> bool;

    /// Finalize session publication after normal or terminal provider shutdown.
    ///
    /// Live telemetry uses this hook to publish `None`, indicating that its
    /// current session is no longer available after the FIFO parser drains.
    ///
    /// This hook is for session-channel state only; implementations should not
    /// attempt to read another frame or refetch session YAML.
    async fn end(&mut self);
}

/// One owned live session snapshot waiting for typed deserialization.
struct SessionParseTask {
    /// Session update counter observed on the frame that triggered the copy.
    observed_version: u32,

    /// Tick of the frame that exposed the changed update counter.
    observed_tick: u32,

    /// Owned YAML copied from the current shared-memory session region.
    yaml: String,
}

/// Tracks changing session information for a live telemetry source.
///
/// Live session YAML is associated with the session version reported by each
/// frame. The policy fetches and owns the current YAML as soon as it observes a
/// version for the first time, then enqueues that snapshot for parsing so the
/// frame reader is not blocked by typed YAML deserialization.
///
/// A single background worker parses queued snapshots one at a time. The queue
/// and worker therefore preserve observation order without generation tracking
/// or concurrent parse-result coordination.
///
/// [`SessionPolicy::end`] closes the task queue, waits for the worker to drain
/// every owned snapshot, and then publishes `None`. A version remains considered
/// observed when fetching returns no YAML or an error, preserving the existing
/// no-retry behavior for repeated frames.
pub(crate) struct LiveSessionPolicy {
    /// Most recently observed frame session version.
    last_version: Option<u32>,

    /// FIFO sender for owned session snapshots awaiting parsing.
    tasks: Option<mpsc::UnboundedSender<SessionParseTask>>,

    /// Latest live session snapshot, cleared after the parser drains.
    sessions: watch::Sender<Option<Arc<SessionInfo>>>,

    /// Background FIFO parser.
    parser: Option<JoinHandle<()>>,

    /// Whether terminal live session state has already been published.
    ended: bool,
}

impl LiveSessionPolicy {
    /// Create a live policy with no previously observed session version.
    pub(crate) fn new(sessions: watch::Sender<Option<Arc<SessionInfo>>>) -> Self {
        let (tasks, task_receiver) = mpsc::unbounded_channel();
        let parser = tokio::spawn(Self::parser_task(task_receiver, sessions.clone()));

        Self {
            last_version: None,
            tasks: Some(tasks),
            sessions,
            parser: Some(parser),
            ended: false,
        }
    }

    async fn parser_task(
        mut tasks: mpsc::UnboundedReceiver<SessionParseTask>,
        sessions: watch::Sender<Option<Arc<SessionInfo>>>,
    ) {
        while let Some(task) = tasks.recv().await {
            let SessionParseTask {
                observed_version,
                observed_tick,
                yaml,
            } = task;

            // Await each blocking parse before receiving another task. Parsing
            // remains off the async runtime workers while FIFO completion is a
            // structural property of this single consumer.
            match tokio::task::spawn_blocking(move || SessionInfo::parse(&yaml)).await {
                Ok(Ok(session)) => {
                    tracing::debug!(
                        version = observed_version,
                        tick = observed_tick,
                        "Parsed live session info"
                    );
                    let _ = sessions.send(Some(Arc::new(session)));
                }
                Ok(Err(error)) => {
                    tracing::warn!(
                        version = observed_version,
                        tick = observed_tick,
                        %error,
                        "Failed to parse session YAML"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        version = observed_version,
                        tick = observed_tick,
                        %error,
                        "Live session parse task failed"
                    );
                }
            }
        }
    }
}

#[async_trait]
impl<P: Provider> SessionPolicy<P> for LiveSessionPolicy {
    async fn observe(
        &mut self,
        provider: &mut P,
        frame: &FramePacket,
        cancel: &CancellationToken,
    ) -> bool {
        // A repeated version uses the already-published session snapshot and
        // does not ask the live provider for identical YAML again.
        let version = frame.session_version;
        if self.last_version == Some(version) {
            return true;
        }

        tracing::debug!(
            "Session version changed: {:#?} -> {}",
            self.last_version,
            version
        );

        // A version is marked observed after any fetch outcome. The policy
        // therefore makes at most one fetch attempt per observed version.
        self.last_version = Some(version);

        // Use select to allow cancellation during provider.session_yaml()
        let result = tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("Frame reader cancelled during session YAML acquisition");
                return false;
            }
            result = provider.session_yaml(version) => result,
        };

        match result {
            Ok(Some(yaml)) => {
                tracing::debug!(
                    "Fetched session YAML ({} bytes) for v{}",
                    yaml.len(),
                    version
                );

                let Some(tasks) = self.tasks.as_ref() else {
                    tracing::warn!("Live session parser is already shut down");
                    return false;
                };

                if tasks
                    .send(SessionParseTask {
                        observed_version: version,
                        observed_tick: frame.tick,
                        yaml,
                    })
                    .is_err()
                {
                    tracing::warn!("Live session parser stopped before accepting YAML");
                    return false;
                }
            }
            Ok(None) => {
                tracing::debug!("No session YAML for version {}", version);
            }
            Err(e) => {
                tracing::warn!("Failed to get session YAML: {}", e);
            }
        }

        true
    }

    async fn end(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;

        // Dropping the sole sender closes the FIFO after all queued tasks. The
        // worker drains those tasks before terminal None is published below.
        self.tasks.take();

        if let Some(handle) = self.parser.take()
            && let Err(error) = handle.await
        {
            tracing::warn!(%error, "Live session parser failed during shutdown");
        }

        // The parser can no longer publish after its handle completes, so None
        // is the stable terminal state even when end is called repeatedly.
        let _ = self.sessions.send(None);
    }
}

impl Drop for LiveSessionPolicy {
    fn drop(&mut self) {
        self.tasks.take();
        if let Some(parser) = self.parser.take() {
            parser.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    //! Live FIFO session parsing characterization.

    use std::time::Duration;

    use tokio::sync::watch;

    use crate::{FramePacket, Result, provider::Provider};

    use super::{LiveSessionPolicy, SessionParseTask, SessionPolicy};

    /// Create valid YAML with a distinctive value for publication assertions.
    fn named_session_yaml(track_name: &str) -> String {
        include_str!("../../../../test-data/session-yaml/profile_small.yaml").replace(
            "TrackName: generated small",
            &format!("TrackName: {track_name}"),
        )
    }

    /// Enqueue an owned YAML snapshot directly into the live FIFO parser.
    fn enqueue_parse(policy: &LiveSessionPolicy, version: u32, tick: u32, yaml: String) {
        policy
            .tasks
            .as_ref()
            .expect("the live parser should still be accepting tasks")
            .send(SessionParseTask {
                observed_version: version,
                observed_tick: tick,
                yaml,
            })
            .expect("the live parser should accept the session snapshot");
    }

    #[tokio::test]
    async fn live_parser_parses_submitted_yaml_and_publishes_the_result() {
        let (sessions, mut session_receiver) = watch::channel(None);
        let mut policy = LiveSessionPolicy::new(sessions);

        enqueue_parse(&policy, 1, 10, named_session_yaml("published"));

        tokio::time::timeout(Duration::from_secs(1), async {
            while session_receiver.borrow_and_update().is_none() {
                session_receiver
                    .changed()
                    .await
                    .expect("the live session channel should remain connected");
            }
        })
        .await
        .expect("background parsing should publish the session promptly");

        assert_eq!(
            session_receiver
                .borrow()
                .as_ref()
                .expect("the parsed session should publish")
                .weekend_info
                .track_name,
            "published"
        );

        <LiveSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;
    }

    #[tokio::test]
    async fn live_end_drains_fifo_tasks_and_is_idempotent() {
        let (sessions, receiver) = watch::channel(None);
        let mut policy = LiveSessionPolicy::new(sessions);

        enqueue_parse(&policy, 1, 10, named_session_yaml("older"));
        enqueue_parse(&policy, 2, 11, named_session_yaml("newer"));

        policy.tasks.take();
        policy
            .parser
            .take()
            .expect("the live parser should still be running")
            .await
            .expect("the live parser should drain its FIFO queue");

        assert_eq!(
            receiver
                .borrow()
                .as_ref()
                .expect("the final queued snapshot should publish")
                .weekend_info
                .track_name,
            "newer",
            "FIFO parsing should leave the second queued snapshot current"
        );

        <LiveSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;
        <LiveSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;

        assert!(
            receiver.borrow().is_none(),
            "end should drain queued tasks and leave terminal session state"
        );
        assert!(
            policy.tasks.is_none(),
            "the stopped parser must reject any later snapshots"
        );
    }

    #[tokio::test]
    async fn live_parser_continues_after_an_earlier_parse_failure() {
        let (sessions, receiver) = watch::channel(None);
        let mut policy = LiveSessionPolicy::new(sessions);

        enqueue_parse(&policy, 1, 10, "not: [valid".to_owned());
        enqueue_parse(&policy, 2, 11, named_session_yaml("recovered"));

        // Closing and awaiting the parser proves both queued tasks settled in
        // FIFO order. The successful second task must not be blocked by the
        // malformed first snapshot.
        policy.tasks.take();
        let parser = policy
            .parser
            .take()
            .expect("the live parser should still be running");
        parser
            .await
            .expect("the live parser should stop after draining its queue");

        assert_eq!(
            receiver
                .borrow()
                .as_ref()
                .expect("the valid later snapshot should publish")
                .weekend_info
                .track_name,
            "recovered"
        );

        <LiveSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;
        assert!(receiver.borrow().is_none());
    }

    struct SessionProvider;
    #[async_trait::async_trait]
    impl Provider for SessionProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            Ok(None)
        }
        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(None)
        }
        fn tick_rate(&self) -> f64 {
            60.0
        }
    }
}
