use std::{sync::Arc, time::Duration};

use futures_util::{StreamExt, stream::BoxStream};
use iracing_sdk::{
    DynamicFrame, LiveConnection, UpdateRate, WindowsConnection, providers::live::LiveProvider,
    schema::SessionInfo,
};

use crate::ports::{
    BoxError, ConnectionAttempt, ProcessProbe, SimulationProbe, TelemetryFactory, TelemetryFeed,
};

pub(crate) struct WindowsProcessProbe;

#[async_trait::async_trait]
impl ProcessProbe for WindowsProcessProbe {
    async fn is_running(&self) -> Result<bool, BoxError> {
        tokio::task::spawn_blocking(iracing_simulation::is_iracing_process_running)
            .await
            .map_err(|error| Box::new(error) as BoxError)?
            .map_err(|error| Box::new(error) as BoxError)
    }
}

pub(crate) struct LocalSimulationProbe;

#[async_trait::async_trait]
impl SimulationProbe for LocalSimulationProbe {
    async fn is_running(&self) -> Result<bool, BoxError> {
        tokio::task::spawn_blocking(|| iracing_simulation::Simulation::local().check_sim_status())
            .await
            .map_err(|error| Box::new(error) as BoxError)
    }
}

pub(crate) struct WindowsTelemetryFactory;

struct WindowsTelemetryFeed {
    connection: LiveConnection,
}

impl TelemetryFeed for WindowsTelemetryFeed {
    fn frames(&self) -> Result<BoxStream<'static, DynamicFrame>, BoxError> {
        self.connection
            .subscribe::<DynamicFrame>(UpdateRate::Native)
            .map(StreamExt::boxed)
            .map_err(|error| Box::new(error) as BoxError)
    }

    fn sessions(&self) -> BoxStream<'static, Arc<SessionInfo>> {
        self.connection.session_updates().boxed()
    }
}

#[async_trait::async_trait]
impl TelemetryFactory for WindowsTelemetryFactory {
    async fn connect(&self) -> Result<ConnectionAttempt, BoxError> {
        let windows =
            WindowsConnection::try_connect().map_err(|error| Box::new(error) as BoxError)?;

        if !windows.is_connected() {
            return Ok(ConnectionAttempt::NotConnected);
        }

        let provider = LiveProvider::builder()
            .with_connection(windows)
            .with_poll_interval(Duration::from_millis(500))
            .with_max_no_connection_attempts(1)
            .build()
            .map_err(|error| Box::new(error) as BoxError)?;

        let connection = LiveConnection::builder()
            .with_provider(provider)
            .build()
            .map_err(|error| Box::new(error) as BoxError)?;

        Ok(ConnectionAttempt::Connected(Box::new(
            WindowsTelemetryFeed { connection },
        )))
    }
}
