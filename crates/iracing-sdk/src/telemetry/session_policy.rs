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
/// Frame delivery and session discovery have different lifecycle requirements.
/// Live telemetry can publish a new session whenever the session version changes,
/// while an IBT file contains one immutable session that can be loaded before any
/// frame is read. Implementations of this policy keep those source-specific rules
/// out of the shared telemetry read loop.
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
    /// Recorded telemetry overrides this method to fetch and publish the file's
    /// immutable session even when the file contains no frames.
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
    /// session version to decide whether to fetch updated YAML. An IBT policy
    /// performs no work here because initialization already loaded its single
    /// immutable session.
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
    /// current session is no longer available. IBT telemetry intentionally
    /// retains its published session because the metadata continues to describe
    /// the completed recording after EOF.
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

/// Exactly-once initialization state for an IBT file's immutable session.
///
/// Every state other than [`IbtSessionState::Pending`] is terminal for this
/// policy instance. Initialization never retries unavailable YAML, provider
/// errors, or parse errors.
#[derive(Debug, PartialEq, Eq)]
enum IbtSessionState {
    /// No session fetch has been attempted.
    Pending,

    /// The one allowed fetch attempt has started.
    Fetching,

    /// YAML was fetched, parsed, and published successfully.
    Published,

    /// The provider reported that the file contains no session YAML.
    Unavailable,

    /// The provider failed while fetching session YAML.
    FetchFailed,

    /// YAML was fetched but could not be parsed as session information.
    ParseFailed,
}

/// Loads and publishes the single immutable session stored in an IBT file.
///
/// Unlike live telemetry, IBT session metadata does not change between frames.
/// This policy performs its work during [`SessionPolicy::initialize`], before
/// frame reading begins, so session metadata is available even for a zero-frame
/// recording. Frame observation is consequently a no-op.
///
/// Initialization makes at most one fetch attempt. The policy parses YAML
/// inline rather than spawning a task, which makes publication deterministic
/// before replay frames are delivered. At EOF, the published session remains
/// available because it still describes the completed recording.
pub(crate) struct IbtSessionPolicy {
    /// Latest-value channel used by IBT session observers.
    sessions: watch::Sender<Option<Arc<SessionInfo>>>,

    /// Current exactly-once initialization state.
    state: IbtSessionState,
}

impl IbtSessionPolicy {
    /// Create an IBT policy that has not attempted to load session YAML.
    pub(crate) fn new(sessions: watch::Sender<Option<Arc<SessionInfo>>>) -> Self {
        Self {
            sessions,
            state: IbtSessionState::Pending,
        }
    }
}

#[async_trait]
impl<P: Provider> SessionPolicy<P> for IbtSessionPolicy {
    async fn initialize(&mut self, provider: &mut P, cancel: &CancellationToken) -> bool {
        // All non-pending states are terminal. Repeated initialization calls
        // are harmless and never ask the provider for session YAML again.
        if self.state != IbtSessionState::Pending {
            return true;
        }

        // Avoid beginning the one allowed attempt when shutdown has already
        // been requested.
        if cancel.is_cancelled() {
            return false;
        }

        // IBT files contain one immutable session YAML document. Mark the
        // policy as started before awaiting it so this lifecycle can make at
        // most one fetch attempt.
        self.state = IbtSessionState::Fetching;

        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => return false,
            // IbtProvider ignores the version argument because an IBT file has
            // one header-level session YAML document.
            result = provider.session_yaml(0) => result,
        };

        match result {
            Ok(Some(yaml)) => {
                tracing::debug!("Fetched IBT session YAML ({} bytes)", yaml.len());

                match SessionInfo::parse(&yaml) {
                    Ok(session) => {
                        tracing::debug!("Parsed IBT session info");
                        let _ = self.sessions.send(Some(Arc::new(session)));
                        self.state = IbtSessionState::Published;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse IBT session YAML: {}", e);
                        self.state = IbtSessionState::ParseFailed;
                    }
                }
            }
            Ok(None) => {
                tracing::debug!("No IBT session YAML");
                self.state = IbtSessionState::Unavailable;
            }
            Err(e) => {
                tracing::warn!("Failed to get IBT session YAML: {}", e);
                self.state = IbtSessionState::FetchFailed;
            }
        }

        true
    }

    async fn observe(
        &mut self,
        _provider: &mut P,
        _frame: &FramePacket,
        _cancel: &CancellationToken,
    ) -> bool {
        // IBT session information was settled during initialization and cannot
        // change as the reader advances or seeks through frames.
        true
    }

    async fn end(&mut self) {
        // Do not publish None: immutable IBT session metadata remains useful
        // after the final recorded frame has been consumed.
    }
}

#[cfg(test)]
mod tests {
    //! Characterization tests for live FIFO parsing and immutable IBT
    //! session handling.
    //!
    //! The tests use [`SessionProvider`] instead of an `IbtReader` so they can
    //! independently control the three provider outcomes relevant to the
    //! policy: YAML, no YAML, and a fetch error. The mock also counts fetches,
    //! allowing each test to enforce the policy's central guarantee that one
    //! initialization lifecycle makes at most one session request.
    //!
    //! Frame bytes and telemetry variables are deliberately irrelevant here.
    //! The only frame passed to `observe` is a minimal packet used to prove that
    //! normal replay traversal does not trigger another session fetch.

    use std::{collections::HashMap, sync::Arc, time::Duration};

    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;

    use crate::{FramePacket, IRacingSDKError, Result, VariableSchema, provider::Provider};

    use super::{
        IbtSessionPolicy, IbtSessionState, LiveSessionPolicy, SessionParseTask, SessionPolicy,
    };

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

    /// Configures the result returned by the mock provider's session endpoint.
    enum SessionOutcome {
        /// Return the supplied YAML text.
        Yaml(&'static str),

        /// Report that no session YAML is available.
        Unavailable,

        /// Return a provider-level fetch failure.
        Error,
    }

    /// A provider that records session fetches without requiring telemetry
    /// frames. This models the fact that IBT session YAML belongs to the file,
    /// not to any individual frame.
    struct SessionProvider {
        /// Result returned by every hypothetical session fetch.
        outcome: SessionOutcome,

        /// Number of times the policy called `session_yaml`.
        fetch_count: usize,
    }

    #[async_trait::async_trait]
    impl Provider for SessionProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            // Session initialization must not depend on a frame being present.
            Ok(None)
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            self.fetch_count += 1;

            match self.outcome {
                SessionOutcome::Yaml(yaml) => Ok(Some(yaml.to_owned())),
                SessionOutcome::Unavailable => Ok(None),
                SessionOutcome::Error => {
                    Err(IRacingSDKError::connection_failed("session fetch failed"))
                }
            }
        }

        fn tick_rate(&self) -> f64 {
            // Tick rate is required by Provider but irrelevant to session
            // initialization.
            60.0
        }
    }

    /// Construct a minimal frame for exercising the no-op `observe` path.
    fn frame() -> FramePacket {
        FramePacket::new(
            Vec::new(),
            0,
            0,
            Arc::new(
                VariableSchema::new(HashMap::new(), 0)
                    .expect("an empty telemetry schema should be valid"),
            ),
        )
    }

    #[tokio::test]
    async fn ibt_initialization_fetches_and_publishes_session_without_a_frame() {
        // Arrange a provider containing known-good fixture YAML but no frames.
        let (sessions, receiver) = watch::channel(None);
        let mut policy = IbtSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome: SessionOutcome::Yaml(include_str!(
                "../../../../test-data/session-yaml/profile_small.yaml"
            )),
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();

        // Act: initialize directly, before any call to next_frame or observe.
        assert!(policy.initialize(&mut provider, &cancel).await);

        // Assert: one fetch produced a parsed session snapshot.
        assert_eq!(provider.fetch_count, 1);
        assert_eq!(policy.state, IbtSessionState::Published);
        assert!(
            receiver.borrow().is_some(),
            "IBT session metadata should be available before any frame is read"
        );
    }

    #[tokio::test]
    async fn ibt_initialization_and_observation_never_refetch_session() {
        // Arrange a provider whose first and only fetch reports no YAML.
        let (sessions, _receiver) = watch::channel(None);
        let mut policy = IbtSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome: SessionOutcome::Unavailable,
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();

        // Act: repeat both lifecycle entry points that could otherwise be
        // mistaken for opportunities to retry.
        assert!(policy.initialize(&mut provider, &cancel).await);
        assert!(policy.initialize(&mut provider, &cancel).await);
        assert!(policy.observe(&mut provider, &frame(), &cancel).await);
        assert!(policy.observe(&mut provider, &frame(), &cancel).await);

        // Assert: the unavailable outcome is terminal and fetched only once.
        assert_eq!(provider.fetch_count, 1);
        assert_eq!(policy.state, IbtSessionState::Unavailable);
    }

    /// Assert that a failed or unavailable initialization outcome is terminal.
    ///
    /// Calling `initialize` twice is intentional: it proves the second call
    /// observes stored state rather than invoking the provider again.
    async fn assert_terminal_initialization_state(
        outcome: SessionOutcome,
        expected: IbtSessionState,
    ) {
        let (sessions, _receiver) = watch::channel(None);
        let mut policy = IbtSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome,
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();

        assert!(policy.initialize(&mut provider, &cancel).await);
        assert!(policy.initialize(&mut provider, &cancel).await);

        assert_eq!(provider.fetch_count, 1);
        assert_eq!(policy.state, expected);
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_retry_an_unavailable_session() {
        // A valid "no YAML" response settles the lifecycle as unavailable.
        assert_terminal_initialization_state(
            SessionOutcome::Unavailable,
            IbtSessionState::Unavailable,
        )
        .await;
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_retry_a_fetch_failure() {
        // Provider errors are recorded once rather than retried on later calls.
        assert_terminal_initialization_state(SessionOutcome::Error, IbtSessionState::FetchFailed)
            .await;
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_retry_a_parse_failure() {
        // Malformed YAML reaches the parser but cannot cause another fetch.
        assert_terminal_initialization_state(
            SessionOutcome::Yaml("not: [valid"),
            IbtSessionState::ParseFailed,
        )
        .await;
    }

    #[tokio::test]
    async fn ibt_end_retains_the_published_session() {
        // Arrange and initialize a successfully parsed IBT session.
        let (sessions, receiver) = watch::channel(None);
        let mut policy = IbtSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome: SessionOutcome::Yaml(include_str!(
                "../../../../test-data/session-yaml/profile_small.yaml"
            )),
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();

        assert!(policy.initialize(&mut provider, &cancel).await);

        // Act: invoke the same end hook used when replay reaches EOF.
        <IbtSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;

        // Assert: EOF does not erase metadata describing the recording.
        assert!(
            receiver.borrow().is_some(),
            "replay EOF should not erase immutable IBT session metadata"
        );
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_fetch_after_cancellation() {
        // Arrange a policy whose cancellation token is already cancelled.
        let (sessions, _receiver) = watch::channel(None);
        let mut policy = IbtSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome: SessionOutcome::Unavailable,
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();
        cancel.cancel();

        // Act and assert: initialization stops before consuming its one allowed
        // fetch attempt, leaving the policy pending and the provider untouched.
        assert!(!policy.initialize(&mut provider, &cancel).await);
        assert_eq!(provider.fetch_count, 0);
        assert_eq!(policy.state, IbtSessionState::Pending);
    }
}
