use super::BroadcastService;

use iracing_sdk::{Broadcast as BroadcastClient, IRacingSDKError};

#[derive(Debug, Default)]
pub struct BroadcastServiceBuilder {
    client: Option<BroadcastClient>,
}

impl BroadcastServiceBuilder {
    pub fn with_client(mut self, client: BroadcastClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let client = match self.client {
            Some(client) => client,
            None => BroadcastClient::new()?,
        };

        Ok(BroadcastService { client })
    }
}
