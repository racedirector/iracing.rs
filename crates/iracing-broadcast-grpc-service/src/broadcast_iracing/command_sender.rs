use async_trait::async_trait;
use iracing_sdk::{Broadcast as BroadcastClient, BroadcastCommand};

use crate::broadcast_app::{BroadcastCommandPort, BroadcastError};

pub(crate) struct IracingBroadcastCommandSender {
    client: BroadcastClient,
}

impl IracingBroadcastCommandSender {
    pub(crate) fn new() -> iracing_sdk::Result<Self> {
        Ok(Self {
            client: BroadcastClient::new()?,
        })
    }

    pub(crate) fn with_client(client: BroadcastClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl BroadcastCommandPort for IracingBroadcastCommandSender {
    async fn send(&self, command: BroadcastCommand) -> Result<(), BroadcastError> {
        self.client.send_message(command)?;
        Ok(())
    }
}
