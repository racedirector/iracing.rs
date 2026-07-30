use std::{sync::Arc, time::Duration};

use crate::{
    broadcast_app::{
        BroadcastCommandPort, BroadcastUseCases, CameraStatePort, DisabledObservationPort,
        ForceFeedbackStatePort, PitStatePort, ReplayStatePort, TelemetryStatePort,
    },
    broadcast_iracing::{IracingBroadcastCommandSender, IracingObservation},
};

use super::{BroadcastService, DEFAULT_OBSERVATION_TIMEOUT};
use iracing_sdk::{Broadcast as BroadcastClient, IRacingSDKError, providers::live::LiveProvider};

/// Builder for a Windows live [`BroadcastService`].
///
/// By default the service opens both the iRacing broadcast command channel and
/// live telemetry observation. Observation is required for RPCs that return
/// telemetry-confirmed state, such as camera switch and replay speed changes.
pub struct BroadcastServiceBuilder {
    sender: Option<Arc<dyn BroadcastCommandPort>>,
    live_provider: Option<LiveProvider>,
    observation_enabled: bool,
    observation_timeout: Duration,
}

struct ObservationPorts {
    camera: Arc<dyn CameraStatePort>,
    replay: Arc<dyn ReplayStatePort>,
    pit: Arc<dyn PitStatePort>,
    telemetry: Arc<dyn TelemetryStatePort>,
    force_feedback: Arc<dyn ForceFeedbackStatePort>,
}

impl Default for BroadcastServiceBuilder {
    fn default() -> Self {
        Self {
            sender: None,
            live_provider: None,
            observation_enabled: true,
            observation_timeout: DEFAULT_OBSERVATION_TIMEOUT,
        }
    }
}

impl BroadcastServiceBuilder {
    /// Use an already-created iRacing broadcast client for command delivery.
    ///
    /// This is useful when the caller wants to control client construction or
    /// share startup error handling. Observation wiring is unchanged.
    pub fn with_client(mut self, client: BroadcastClient) -> Self {
        self.sender = Some(Arc::new(IracingBroadcastCommandSender::with_client(client)));
        self
    }

    /// Use an already-created live telemetry provider for observation.
    ///
    /// This lets the caller control `LiveProvider` construction before handing
    /// it to the gRPC service. The provider is ignored when observation is
    /// disabled with [`Self::without_observation`].
    pub fn with_live_provider(mut self, provider: LiveProvider) -> Self {
        self.live_provider = Some(provider);
        self
    }

    /// Set how long telemetry-backed RPCs wait for the requested state.
    ///
    /// The default timeout is two seconds. The timeout applies after the
    /// command has been sent and only affects operations that observe telemetry.
    pub fn with_observation_timeout(mut self, timeout: Duration) -> Self {
        self.observation_timeout = timeout;
        self
    }

    /// Disable telemetry observation and expose only ack-style command RPCs.
    ///
    /// RPCs that require state confirmation will return `FAILED_PRECONDITION`.
    /// This mode avoids opening live telemetry while still allowing commands
    /// such as chat, reload textures, replay state, and video capture.
    pub fn without_observation(mut self) -> Self {
        self.observation_enabled = false;
        self
    }

    /// Build the configured service.
    ///
    /// Returns an SDK error if the broadcast command channel cannot be opened,
    /// or if observation is enabled and live telemetry cannot be initialized.
    pub fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let sender = match self.sender {
            Some(sender) => sender,
            None => Arc::new(IracingBroadcastCommandSender::new()?),
        };

        let observation = if !self.observation_enabled {
            let disabled = Arc::new(DisabledObservationPort);
            ObservationPorts {
                camera: disabled.clone(),
                replay: disabled.clone(),
                pit: disabled.clone(),
                telemetry: disabled.clone(),
                force_feedback: disabled,
            }
        } else {
            let observation = Arc::new(match self.live_provider {
                Some(provider) => {
                    let schema = provider.schema();
                    IracingObservation::from_provider(provider, schema)
                }
                None => IracingObservation::live()?,
            });
            ObservationPorts {
                camera: observation.clone(),
                replay: observation.clone(),
                pit: observation.clone(),
                telemetry: observation.clone(),
                force_feedback: observation,
            }
        };

        Ok(BroadcastService::from_use_cases(Arc::new(
            BroadcastUseCases::new(
                sender,
                observation.camera,
                observation.replay,
                observation.pit,
                observation.telemetry,
                observation.force_feedback,
                self.observation_timeout,
            ),
        )))
    }
}
