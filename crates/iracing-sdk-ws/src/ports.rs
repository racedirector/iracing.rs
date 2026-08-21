use std::sync::Arc;

use futures_util::stream::BoxStream;
use iracing_sdk::{DynamicFrame, schema::SessionInfo};

/// Error returned by an injected runtime dependency.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Reports whether the iRacing simulation process is present.
#[async_trait::async_trait]
pub trait ProcessProbe: Send + Sync {
    async fn is_running(&self) -> Result<bool, BoxError>;
}

/// Reports whether the iRacing simulation endpoint is ready.
#[async_trait::async_trait]
pub trait SimulationProbe: Send + Sync {
    async fn is_running(&self) -> Result<bool, BoxError>;
}

/// Result of one attempt to create a live telemetry source.
pub enum ConnectionAttempt {
    /// Shared memory opened, but the SDK connected flag is not set.
    NotConnected,
    /// A live source was created successfully.
    Connected(Box<dyn TelemetryFeed>),
}

/// Creates the single live telemetry source owned by the service runtime.
#[async_trait::async_trait]
pub trait TelemetryFactory: Send + Sync {
    async fn connect(&self) -> Result<ConnectionAttempt, BoxError>;
}

/// Consumer-facing streams from one owned live telemetry connection.
pub trait TelemetryFeed: Send {
    fn frames(&self) -> Result<BoxStream<'static, DynamicFrame>, BoxError>;
    fn sessions(&self) -> BoxStream<'static, Arc<SessionInfo>>;
}
