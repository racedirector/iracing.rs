//! Immutable session initialization retained for historical pipeline measurements.
use crate::{
    FramePacket, provider::Provider, schema::SessionInfo, telemetry::session_policy::SessionPolicy,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
/// Exactly-once initialization state for an IBT file's immutable session.
///
/// Every state other than [`RecordedSessionState::Pending`] is terminal for this
/// policy instance. Initialization never retries unavailable YAML, provider
/// errors, or parse errors.
#[derive(Debug, PartialEq, Eq)]
enum RecordedSessionState {
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
pub(crate) struct RecordedSessionPolicy {
    /// Latest-value channel used by IBT session observers.
    sessions: watch::Sender<Option<Arc<SessionInfo>>>,

    /// Current exactly-once initialization state.
    pub(self) state: RecordedSessionState,
}

impl RecordedSessionPolicy {
    /// Create an IBT policy that has not attempted to load session YAML.
    pub(crate) fn new(sessions: watch::Sender<Option<Arc<SessionInfo>>>) -> Self {
        Self {
            sessions,
            state: RecordedSessionState::Pending,
        }
    }
}

#[async_trait]
impl<P: Provider> SessionPolicy<P> for RecordedSessionPolicy {
    async fn initialize(&mut self, provider: &mut P, cancel: &CancellationToken) -> bool {
        // All non-pending states are terminal. Repeated initialization calls
        // are harmless and never ask the provider for session YAML again.
        if self.state != RecordedSessionState::Pending {
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
        self.state = RecordedSessionState::Fetching;

        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => return false,
            // The recorded provider ignores the version argument because an IBT file has
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
                        self.state = RecordedSessionState::Published;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse IBT session YAML: {}", e);
                        self.state = RecordedSessionState::ParseFailed;
                    }
                }
            }
            Ok(None) => {
                tracing::debug!("No IBT session YAML");
                self.state = RecordedSessionState::Unavailable;
            }
            Err(e) => {
                tracing::warn!("Failed to get IBT session YAML: {}", e);
                self.state = RecordedSessionState::FetchFailed;
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
    use super::*;
    use crate::{IRacingSDKError, Result};
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
            vec![0],
            0,
            0,
            Arc::new(
                crate::test_utils::layout([], 1)
                    .expect("an empty telemetry layout should be valid"),
            ),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn ibt_initialization_fetches_and_publishes_session_without_a_frame() {
        // Arrange a provider containing known-good fixture YAML but no frames.
        let (sessions, receiver) = watch::channel(None);
        let mut policy = RecordedSessionPolicy::new(sessions);
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
        assert_eq!(policy.state, RecordedSessionState::Published);
        assert!(
            receiver.borrow().is_some(),
            "IBT session metadata should be available before any frame is read"
        );
    }

    #[tokio::test]
    async fn ibt_initialization_and_observation_never_refetch_session() {
        // Arrange a provider whose first and only fetch reports no YAML.
        let (sessions, _receiver) = watch::channel(None);
        let mut policy = RecordedSessionPolicy::new(sessions);
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
        assert_eq!(policy.state, RecordedSessionState::Unavailable);
    }

    /// Assert that a failed or unavailable initialization outcome is terminal.
    ///
    /// Calling `initialize` twice is intentional: it proves the second call
    /// observes stored state rather than invoking the provider again.
    async fn assert_terminal_initialization_state(
        outcome: SessionOutcome,
        expected: RecordedSessionState,
    ) {
        let (sessions, _receiver) = watch::channel(None);
        let mut policy = RecordedSessionPolicy::new(sessions);
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
            RecordedSessionState::Unavailable,
        )
        .await;
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_retry_a_fetch_failure() {
        // Provider errors are recorded once rather than retried on later calls.
        assert_terminal_initialization_state(
            SessionOutcome::Error,
            RecordedSessionState::FetchFailed,
        )
        .await;
    }

    #[tokio::test]
    async fn ibt_initialization_does_not_retry_a_parse_failure() {
        // Malformed YAML reaches the parser but cannot cause another fetch.
        assert_terminal_initialization_state(
            SessionOutcome::Yaml("not: [valid"),
            RecordedSessionState::ParseFailed,
        )
        .await;
    }

    #[tokio::test]
    async fn ibt_end_retains_the_published_session() {
        // Arrange and initialize a successfully parsed IBT session.
        let (sessions, receiver) = watch::channel(None);
        let mut policy = RecordedSessionPolicy::new(sessions);
        let mut provider = SessionProvider {
            outcome: SessionOutcome::Yaml(include_str!(
                "../../../../test-data/session-yaml/profile_small.yaml"
            )),
            fetch_count: 0,
        };
        let cancel = CancellationToken::new();

        assert!(policy.initialize(&mut provider, &cancel).await);

        // Act: invoke the same end hook used when replay reaches EOF.
        <RecordedSessionPolicy as SessionPolicy<SessionProvider>>::end(&mut policy).await;

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
        let mut policy = RecordedSessionPolicy::new(sessions);
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
        assert_eq!(policy.state, RecordedSessionState::Pending);
    }
}
