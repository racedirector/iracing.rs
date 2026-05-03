//! IBT connection for IBT files

use futures::{Stream, StreamExt, stream};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use crate::providers::IbtProvider;
use crate::providers::Provider;

use crate::{FrameAdapter, Result, SessionInfo, VariableSchema};

/// IBT connection from file
pub struct IbtConnection {
    /// Replay provider consumed by the first subscriber.
    provider: Mutex<Option<IbtProvider>>,

    /// Session watch receiver
    sessions: watch::Receiver<Option<Arc<SessionInfo>>>,

    /// Session watch sender
    session_tx: watch::Sender<Option<Arc<SessionInfo>>>,

    /// Variable schema
    schema: Arc<VariableSchema>,

    /// Source frequency
    source_hz: f64,
}

impl IbtConnection {
    /// Open an IBT file for replay.
    ///
    /// The replay stream is lazy: frames are read only after a subscriber polls
    /// the stream returned from [`Self::subscribe`].
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        tracing::info!("Opening IBT file: {}", path.display());

        // Create provider and extract metadata
        let provider = IbtProvider::from_path(path)?;

        Self::with_provider(provider).await
    }

    /// Creates an IBT connection with a given provider.
    pub async fn with_provider(provider: IbtProvider) -> Result<Self> {
        let schema = provider.schema();
        let source_hz = provider.tick_rate();
        let (session_tx, session_rx) = watch::channel(None);

        tracing::info!("IBT connection opened ({}Hz)", source_hz);

        Ok(Self {
            provider: Mutex::new(Some(provider)),
            sessions: session_rx,
            session_tx,
            schema,
            source_hz,
        })
    }

    /// Subscribe to telemetry frames.
    ///
    /// IBT replay is a finite one-shot stream. The first subscriber consumes the
    /// replay from disk as the stream is polled; additional subscriptions panic.
    pub fn subscribe<T>(&self) -> impl Stream<Item = T> + 'static
    where
        T: FrameAdapter + Send + 'static,
    {
        // Validate schema once at subscription time
        let validation = T::validate_schema(&self.schema).expect("Schema validation failed");

        let provider = self
            .provider
            .lock()
            .expect("IBT provider mutex poisoned")
            .take()
            .expect("IBT replay stream already has a subscriber");
        let session_tx = self.session_tx.clone();

        stream::unfold(
            (provider, None),
            move |(mut provider, mut last_session_version)| {
                let validation = validation.clone();
                let session_tx = session_tx.clone();

                async move {
                    loop {
                        match provider.next_frame().await {
                            Ok(Some(packet)) => {
                                let version = packet.session_version;

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
                                                    let _ =
                                                        session_tx.send(Some(Arc::new(session)));
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Failed to parse session YAML: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            tracing::debug!(
                                                "No session YAML for version {}",
                                                version
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to get session YAML: {}", e);
                                        }
                                    }

                                    last_session_version = Some(version);
                                }

                                let adapted = T::adapt(&packet, &validation);
                                return Some((adapted, (provider, last_session_version)));
                            }
                            Ok(None) => {
                                tracing::debug!("End of IBT replay stream");
                                let _ = session_tx.send(None);
                                return None;
                            }
                            Err(e) => {
                                tracing::error!("Provider error: {}", e);
                                let _ = session_tx.send(None);
                                return None;
                            }
                        }
                    }
                }
            },
        )
        .boxed()
    }

    /// Get session updates as a stream
    pub fn session_updates(&self) -> impl Stream<Item = Arc<SessionInfo>> + 'static {
        // Simply watch the session channel - Driver handles all the complexity!
        WatchStream::new(self.sessions.clone()).filter_map(|opt| async move { opt })
    }

    /// Get current session info (if available)
    pub fn current_session(&self) -> Option<Arc<SessionInfo>> {
        self.sessions.borrow().clone()
    }

    /// Get the source telemetry frequency
    pub fn source_hz(&self) -> f64 {
        self.source_hz
    }

    /// Get the variable schema
    pub fn schema(&self) -> &VariableSchema {
        &self.schema
    }
}

impl Drop for IbtConnection {
    fn drop(&mut self) {
        tracing::debug!("Dropping IBT connection");
    }
}
