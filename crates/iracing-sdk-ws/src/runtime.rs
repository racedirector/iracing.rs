use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use iracing_sdk::DynamicFrame;
use serde::Serialize;
use tokio::{sync::watch, task::JoinHandle};

use crate::ports::{
    ConnectionAttempt, ProcessProbe, SimulationProbe, TelemetryFactory, TelemetryFeed,
};

const RETRY_INTERVAL: Duration = Duration::from_secs(1);

/// Current position in the iRacing startup lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IRacingConnectionStatus {
    Pending,
    ProcessRunning,
    SimulationRunning,
    TelemetryConnected,
}

/// JSON response returned by `/status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub status: IRacingConnectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl StatusSnapshot {
    fn ready(status: IRacingConnectionStatus) -> Self {
        Self {
            status,
            last_error: None,
        }
    }

    fn failed(status: IRacingConnectionStatus, error: impl ToString) -> Self {
        Self {
            status,
            last_error: Some(error.to_string()),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeHandle {
    status: watch::Receiver<StatusSnapshot>,
    frame: watch::Receiver<Option<Arc<DynamicFrame>>>,
    session: watch::Receiver<Option<Arc<serde_json::Value>>>,
}

impl RuntimeHandle {
    pub(crate) fn status(&self) -> StatusSnapshot {
        self.status.borrow().clone()
    }

    pub(crate) fn current_frame(&self) -> Option<Arc<DynamicFrame>> {
        self.frame.borrow().clone()
    }

    pub(crate) fn current_session(&self) -> Option<Arc<serde_json::Value>> {
        self.session.borrow().clone()
    }
}

pub(crate) struct Runtime {
    handle: RuntimeHandle,
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}

impl Runtime {
    pub(crate) fn spawn(
        process_probe: Arc<dyn ProcessProbe>,
        simulation_probe: Arc<dyn SimulationProbe>,
        telemetry_factory: Arc<dyn TelemetryFactory>,
    ) -> Self {
        let (status_tx, status_rx) =
            watch::channel(StatusSnapshot::ready(IRacingConnectionStatus::Pending));
        let (frame_tx, frame_rx) = watch::channel(None);
        let (session_tx, session_rx) = watch::channel(None);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task = tokio::spawn(supervise(
            process_probe,
            simulation_probe,
            telemetry_factory,
            status_tx,
            frame_tx,
            session_tx,
            shutdown_rx,
        ));

        Self {
            handle: RuntimeHandle {
                status: status_rx,
                frame: frame_rx,
                session: session_rx,
            },
            shutdown: shutdown_tx,
            task: Some(task),
        }
    }

    pub(crate) fn handle(&self) -> RuntimeHandle {
        self.handle.clone()
    }

    pub(crate) async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn supervise(
    process_probe: Arc<dyn ProcessProbe>,
    simulation_probe: Arc<dyn SimulationProbe>,
    telemetry_factory: Arc<dyn TelemetryFactory>,
    status: watch::Sender<StatusSnapshot>,
    frame: watch::Sender<Option<Arc<DynamicFrame>>>,
    session: watch::Sender<Option<Arc<serde_json::Value>>>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            break;
        }

        match process_probe.is_running().await {
            Ok(false) => {
                publish(&status, IRacingConnectionStatus::Pending);
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Err(error) => {
                publish_error(&status, IRacingConnectionStatus::Pending, error);
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Ok(true) => publish(&status, IRacingConnectionStatus::ProcessRunning),
        }

        match simulation_probe.is_running().await {
            Ok(false) => {
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Err(error) => {
                publish_error(&status, IRacingConnectionStatus::ProcessRunning, error);
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Ok(true) => publish(&status, IRacingConnectionStatus::SimulationRunning),
        }

        let source = match telemetry_factory.connect().await {
            Ok(ConnectionAttempt::NotConnected) => {
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Err(error) => {
                publish_error(&status, IRacingConnectionStatus::SimulationRunning, error);
                if wait_to_retry(&mut shutdown).await {
                    break;
                }
                continue;
            }
            Ok(ConnectionAttempt::Connected(source)) => source,
        };

        if monitor_source(source, &status, &frame, &session, &mut shutdown).await {
            break;
        }

        let _ = frame.send(None);
        let _ = session.send(None);
        publish(&status, IRacingConnectionStatus::SimulationRunning);
    }

    let _ = frame.send(None);
    let _ = session.send(None);
}

async fn monitor_source(
    source: Box<dyn TelemetryFeed>,
    status: &watch::Sender<StatusSnapshot>,
    frame: &watch::Sender<Option<Arc<DynamicFrame>>>,
    session: &watch::Sender<Option<Arc<serde_json::Value>>>,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    let mut frames = match source.frames() {
        Ok(frames) => frames,
        Err(error) => {
            publish_error(status, IRacingConnectionStatus::SimulationRunning, error);
            return false;
        }
    };
    let mut sessions = source.sessions();

    publish(status, IRacingConnectionStatus::TelemetryConnected);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                return changed.is_err() || *shutdown.borrow();
            }
            next = frames.next() => match next {
                Some(next) => { let _ = frame.send(Some(Arc::new(next))); }
                None => return false,
            },
            next = sessions.next() => match next {
                Some(next) => match serde_json::to_value(next.as_ref()) {
                    Ok(next) => { let _ = session.send(Some(Arc::new(next))); }
                    Err(error) => tracing::warn!(%error, "failed to serialize session snapshot"),
                },
                None => return false,
            },
        }
    }
}

fn publish(status: &watch::Sender<StatusSnapshot>, next: IRacingConnectionStatus) {
    let _ = status.send(StatusSnapshot::ready(next));
}

fn publish_error(
    status: &watch::Sender<StatusSnapshot>,
    next: IRacingConnectionStatus,
    error: impl ToString,
) {
    let _ = status.send(StatusSnapshot::failed(next, error));
}

async fn wait_to_retry(shutdown: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(RETRY_INTERVAL) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures_util::{StreamExt, stream};
    use iracing_sdk::schema::SessionInfo;

    use super::*;
    use crate::ports::{BoxError, ConnectionAttempt};

    struct ReadyProbe;

    #[async_trait::async_trait]
    impl ProcessProbe for ReadyProbe {
        async fn is_running(&self) -> Result<bool, BoxError> {
            Ok(true)
        }
    }

    #[async_trait::async_trait]
    impl SimulationProbe for ReadyProbe {
        async fn is_running(&self) -> Result<bool, BoxError> {
            Ok(true)
        }
    }

    struct PendingFeed;

    impl TelemetryFeed for PendingFeed {
        fn frames(
            &self,
        ) -> Result<futures_util::stream::BoxStream<'static, DynamicFrame>, BoxError> {
            Ok(stream::pending().boxed())
        }

        fn sessions(&self) -> futures_util::stream::BoxStream<'static, Arc<SessionInfo>> {
            stream::pending().boxed()
        }
    }

    struct CountingFactory(AtomicUsize);

    #[async_trait::async_trait]
    impl TelemetryFactory for CountingFactory {
        async fn connect(&self) -> Result<ConnectionAttempt, BoxError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ConnectionAttempt::Connected(Box::new(PendingFeed)))
        }
    }

    #[tokio::test]
    async fn one_factory_connection_reaches_telemetry_connected() {
        let probe = Arc::new(ReadyProbe);
        let factory = Arc::new(CountingFactory(AtomicUsize::new(0)));
        let runtime = Runtime::spawn(probe.clone(), probe, factory.clone());
        let mut status = runtime.handle.status.clone();

        tokio::time::timeout(Duration::from_secs(1), async {
            while status.borrow_and_update().status != IRacingConnectionStatus::TelemetryConnected {
                status.changed().await.expect("runtime status channel");
            }
        })
        .await
        .expect("runtime should connect");

        assert_eq!(factory.0.load(Ordering::SeqCst), 1);
        runtime.shutdown().await;
    }
}
