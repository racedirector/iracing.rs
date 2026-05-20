#[cfg(windows)]
mod broadcast_service;

pub use broadcast::broadcast_client::BroadcastClient as RawBroadcastClient;
pub use broadcast::broadcast_server::{Broadcast, BroadcastServer};
pub use broadcast::*;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use broadcast_service::BroadcastService;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use broadcast_service::BroadcastServiceBuilder;

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("iracing.broadcast");

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}
