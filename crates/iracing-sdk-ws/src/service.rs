use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use tokio::time::{MissedTickBehavior, interval};

use crate::{
    WEBSOCKET_PATH,
    ports::{BoxError, ProcessProbe, SimulationProbe, TelemetryFactory},
    protocol::{KappsRequest, Subscription},
    runtime::{Runtime, RuntimeHandle, StatusSnapshot},
};

#[derive(Clone)]
struct AppState {
    runtime: RuntimeHandle,
}

/// HTTP and WebSocket facade backed by one live telemetry runtime.
pub struct Service {
    path: String,
    runtime: Runtime,
    state: AppState,
}

/// Builder for configuring a [`Service`].
///
/// The WebSocket endpoint defaults to [`WEBSOCKET_PATH`]. On Windows, omitted
/// runtime dependencies use their normal live implementations. All three
/// dependencies must be injected when building on another platform.
pub struct ServiceBuilder {
    path: String,
    process_probe: Option<Arc<dyn ProcessProbe>>,
    simulation_probe: Option<Arc<dyn SimulationProbe>>,
    telemetry_factory: Option<Arc<dyn TelemetryFactory>>,
}

impl Default for ServiceBuilder {
    fn default() -> Self {
        Self {
            path: WEBSOCKET_PATH.to_owned(),
            process_probe: None,
            simulation_probe: None,
            telemetry_factory: None,
        }
    }
}

impl ServiceBuilder {
    /// Override the WebSocket endpoint path.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Override the probe used to detect the iRacing process.
    pub fn process_probe(mut self, probe: Arc<dyn ProcessProbe>) -> Self {
        self.process_probe = Some(probe);
        self
    }

    /// Override the probe used to detect the running simulation.
    pub fn simulation_probe(mut self, probe: Arc<dyn SimulationProbe>) -> Self {
        self.simulation_probe = Some(probe);
        self
    }

    /// Override the factory used to connect to live telemetry.
    pub fn telemetry_factory(mut self, factory: Arc<dyn TelemetryFactory>) -> Self {
        self.telemetry_factory = Some(factory);
        self
    }

    /// Build the configured service.
    pub fn build(self) -> Result<Service, BoxError> {
        let dependencies = match (
            self.process_probe,
            self.simulation_probe,
            self.telemetry_factory,
        ) {
            (Some(process_probe), Some(simulation_probe), Some(telemetry_factory)) => {
                (process_probe, simulation_probe, telemetry_factory)
            }
            dependencies => live_dependencies(dependencies)?,
        };

        Ok(Service::from_parts(
            self.path,
            dependencies.0,
            dependencies.1,
            dependencies.2,
        ))
    }
}

impl Service {
    /// Start configuring a service using the default `/ws` endpoint.
    pub fn builder() -> ServiceBuilder {
        ServiceBuilder::default()
    }

    /// Construct a service with injected lifecycle dependencies.
    pub fn new(
        process_probe: Arc<dyn ProcessProbe>,
        simulation_probe: Arc<dyn SimulationProbe>,
        telemetry_factory: Arc<dyn TelemetryFactory>,
    ) -> Self {
        Self::from_parts(
            WEBSOCKET_PATH.to_owned(),
            process_probe,
            simulation_probe,
            telemetry_factory,
        )
    }

    fn from_parts(
        path: String,
        process_probe: Arc<dyn ProcessProbe>,
        simulation_probe: Arc<dyn SimulationProbe>,
        telemetry_factory: Arc<dyn TelemetryFactory>,
    ) -> Self {
        let runtime = Runtime::spawn(process_probe, simulation_probe, telemetry_factory);
        let state = AppState {
            runtime: runtime.handle(),
        };

        Self {
            path,
            runtime,
            state,
        }
    }

    /// Construct the normal Windows-backed live service.
    pub fn live() -> Result<Self, BoxError> {
        Self::builder().build()
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Build the HTTP router while retaining runtime ownership in this service.
    pub fn router(&self) -> Router {
        tracing::trace!(path = self.path, "registering HTTP routes");
        Router::new()
            .route(&self.path, get(Self::upgrade_websocket))
            .route("/status", get(Self::status))
            .with_state(self.state.clone())
    }

    /// Stop and await the telemetry supervisor.
    pub async fn shutdown(self) {
        self.runtime.shutdown().await;
    }

    async fn status(State(state): State<AppState>) -> Json<StatusSnapshot> {
        Json(state.runtime.status())
    }

    async fn upgrade_websocket(
        State(state): State<AppState>,
        upgrade: WebSocketUpgrade,
    ) -> Response {
        upgrade.on_upgrade(move |socket| Self::handle_socket(socket, state.runtime))
    }

    async fn handle_socket(mut socket: WebSocket, runtime: RuntimeHandle) {
        let mut subscription: Option<Subscription> = None;
        let mut ticker = interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                incoming = socket.recv() => {
                    let Some(incoming) = incoming else { break };
                    let Ok(incoming) = incoming else { break };

                    match incoming {
                        Message::Close(_) => break,
                        Message::Text(text) => {
                            let request = match serde_json::from_str::<KappsRequest>(text.as_str()) {
                                Ok(request) => request,
                                Err(error) => {
                                    tracing::debug!(%error, "invalid WebSocket request");
                                    let _ = socket.send(close(1007, "invalid request")).await;
                                    break;
                                }
                            };

                            if request.read_ibt() {
                                let _ = socket
                                    .send(close(1003, "IBT replay is not supported"))
                                    .await;
                                break;
                            }

                            let next = Subscription::from_request(request);
                            ticker = interval(Duration::from_secs_f64(1.0 / next.fps() as f64));
                            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                            subscription = Some(next);
                        }
                        _ => {}
                    }
                }
                _ = ticker.tick(), if subscription.is_some() => {
                    let frame = runtime.current_frame();
                    let session = runtime.current_session();
                    let Some(response) = subscription
                        .as_mut()
                        .and_then(|subscription| subscription.response(frame.as_ref(), session.as_ref()))
                    else {
                        continue;
                    };

                    let text = match serde_json::to_string(&response) {
                        Ok(text) => text,
                        Err(error) => {
                            tracing::warn!(%error, "failed to serialize WebSocket response");
                            break;
                        }
                    };

                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

type Dependencies = (
    Option<Arc<dyn ProcessProbe>>,
    Option<Arc<dyn SimulationProbe>>,
    Option<Arc<dyn TelemetryFactory>>,
);
type ResolvedDependencies = (
    Arc<dyn ProcessProbe>,
    Arc<dyn SimulationProbe>,
    Arc<dyn TelemetryFactory>,
);

#[cfg(windows)]
fn live_dependencies(dependencies: Dependencies) -> Result<ResolvedDependencies, BoxError> {
    use crate::platform::{LocalSimulationProbe, WindowsProcessProbe, WindowsTelemetryFactory};

    Ok((
        dependencies
            .0
            .unwrap_or_else(|| Arc::new(WindowsProcessProbe)),
        dependencies
            .1
            .unwrap_or_else(|| Arc::new(LocalSimulationProbe)),
        dependencies
            .2
            .unwrap_or_else(|| Arc::new(WindowsTelemetryFactory)),
    ))
}

#[cfg(not(windows))]
fn live_dependencies(_dependencies: Dependencies) -> Result<ResolvedDependencies, BoxError> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "live iRacing telemetry is only available on Windows; inject all runtime dependencies",
    )
    .into())
}

fn close(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConnectionAttempt;

    struct Unavailable;

    #[async_trait::async_trait]
    impl ProcessProbe for Unavailable {
        async fn is_running(&self) -> Result<bool, BoxError> {
            Ok(false)
        }
    }

    #[async_trait::async_trait]
    impl SimulationProbe for Unavailable {
        async fn is_running(&self) -> Result<bool, BoxError> {
            Ok(false)
        }
    }

    #[async_trait::async_trait]
    impl TelemetryFactory for Unavailable {
        async fn connect(&self) -> Result<ConnectionAttempt, BoxError> {
            Ok(ConnectionAttempt::NotConnected)
        }
    }

    fn builder() -> ServiceBuilder {
        Service::builder()
            .process_probe(Arc::new(Unavailable))
            .simulation_probe(Arc::new(Unavailable))
            .telemetry_factory(Arc::new(Unavailable))
    }

    #[tokio::test]
    async fn builder_defaults_to_websocket_path() {
        let service = builder()
            .build()
            .expect("injected dependencies should build");

        assert_eq!(service.path(), WEBSOCKET_PATH);
        service.shutdown().await;
    }

    #[tokio::test]
    async fn builder_accepts_a_custom_path() {
        let service = builder()
            .path("/telemetry")
            .build()
            .expect("injected dependencies should build");

        assert_eq!(service.path(), "/telemetry");
        service.shutdown().await;
    }
}
