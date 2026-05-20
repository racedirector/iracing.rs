use std::{sync::Arc, time::Duration};

use super::{
    BroadcastClient, BroadcastCommandSender, BroadcastService, DEFAULT_OBSERVATION_TIMEOUT,
    ObservationBackend, ServiceObservation,
};
use iracing_sdk::IRacingSDKError;

pub struct BroadcastServiceBuilder {
    sender: Option<Arc<dyn BroadcastCommandSender>>,
    observation: Option<Arc<dyn ObservationBackend>>,
    observation_enabled: bool,
    observation_timeout: Duration,
}

impl Default for BroadcastServiceBuilder {
    fn default() -> Self {
        Self {
            sender: None,
            observation: None,
            observation_enabled: true,
            observation_timeout: DEFAULT_OBSERVATION_TIMEOUT,
        }
    }
}

impl BroadcastServiceBuilder {
    pub fn with_client(mut self, client: BroadcastClient) -> Self {
        self.sender = Some(Arc::new(client));
        self
    }

    pub fn with_observation_timeout(mut self, timeout: Duration) -> Self {
        self.observation_timeout = timeout;
        self
    }

    pub fn without_observation(mut self) -> Self {
        self.observation_enabled = false;
        self
    }

    pub fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let sender = match self.sender {
            Some(sender) => sender,
            None => Arc::new(BroadcastClient::new()?),
        };

        let observation = if !self.observation_enabled {
            None
        } else {
            Some(match self.observation {
                Some(observation) => observation,
                None => Arc::new(ServiceObservation::live()?),
            })
        };

        Ok(BroadcastService {
            sender,
            observation,
            observation_timeout: self.observation_timeout,
        })
    }
}
