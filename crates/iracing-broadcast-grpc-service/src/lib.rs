mod broadcast_service;

pub use broadcast::broadcast_server::{Broadcast, BroadcastServer};
pub use broadcast::*;
pub use broadcast_service::BroadcastService;

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}
