use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;
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
/// 3. [`SessionPolicy::end`] when the provider reaches EOF or exceeds its error
///    limit.
///
/// Cancellation can stop the telemetry task without calling [`SessionPolicy::end`].
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

/// Tracks changing session information for a live telemetry source.
///
/// Live session YAML is associated with the session version reported by each
/// frame. The policy fetches YAML when it observes a version for the first time
/// and spawns parsing so the frame reader is not blocked by YAML parsing.
///
/// The version is considered observed after the fetch attempt, including when
/// the provider returns no YAML or an error. Parsing also happens in a detached
/// task, so parse completion is not ordered with later versions or shutdown.
/// These details intentionally preserve the existing live-session behavior.
pub(crate) struct LiveSessionPolicy {
    /// Latest-value channel used by live session observers.
    sessions: watch::Sender<Option<Arc<SessionInfo>>>,

    /// Most recently observed frame session version.
    last_version: Option<u32>,
}

impl LiveSessionPolicy {
    /// Create a live policy with no previously observed session version.
    pub(crate) fn new(sessions: watch::Sender<Option<Arc<SessionInfo>>>) -> Self {
        Self {
            sessions,
            last_version: None,
        }
    }
}

#[async_trait]
impl<P: Provider> SessionPolicy<P> for LiveSessionPolicy {
    async fn observe(
        &mut self,
        provider: &mut P,
        frame: &FramePacket,
        _cancel: &CancellationToken, // TODO: Implement cancellation handling later.
    ) -> bool {
        // A repeated version uses the already-published session snapshot and
        // does not ask the live provider for identical YAML again.
        let version = frame.session_version;
        if self.last_version != Some(version) {
            tracing::debug!(
                "Session version changed: {} -> {}",
                self.last_version.unwrap_or(0),
                version
            );

            match provider.session_yaml(version).await {
                Ok(Some(yaml)) => {
                    tracing::debug!(
                        "Fetched session YAML ({} bytes) for v{}",
                        yaml.len(),
                        version
                    );

                    // The detached parser owns a sender clone so frame reading
                    // can continue while the YAML is parsed.
                    let session_clone = self.sessions.clone();

                    // This preserves the existing latest-wins live behavior:
                    // independently spawned versions can complete out of order.
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

            // A version is marked observed after any fetch outcome. The policy
            // therefore makes at most one fetch attempt per observed version.
            self.last_version = Some(version);
        }

        true
    }

    async fn end(&mut self) {
        let _ = self.sessions.send(None);
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
    //! Characterization tests for immutable IBT session handling.
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

    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;

    use crate::{FramePacket, IRacingSDKError, Result, VariableSchema, provider::Provider};

    use super::{IbtSessionPolicy, IbtSessionState, SessionPolicy};

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
