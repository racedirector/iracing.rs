#[cfg(windows)]
mod broadcast_service;
mod client;

pub use broadcast::broadcast_client::BroadcastClient as RawBroadcastClient;
pub use broadcast::broadcast_server::{Broadcast, BroadcastServer};
pub use broadcast::*;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use broadcast_service::BroadcastService;
pub use client::{BroadcastGrpcClient, BroadcastGrpcResult};

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}
