//! gRPC service surface for iRacing broadcast controls.
//!
//! The crate exposes generated protobuf request/response types plus a
//! Windows-only `BroadcastService` implementation that sends iRacing broadcast
//! commands and, for supported operations, observes live telemetry until the
//! requested state is visible.
//!
//! On non-Windows platforms the generated protobuf types and tonic client/server
//! traits remain available, but the live `BroadcastService` adapter is not
//! exported because iRacing's broadcast transport depends on Win32 APIs.

#[cfg(windows)]
mod broadcast_app;
#[cfg(windows)]
mod broadcast_iracing;
#[cfg(windows)]
mod broadcast_service;
#[cfg(windows)]
mod telemetry_observer;

/// Generated tonic client for the raw broadcast gRPC service.
pub use broadcast::broadcast_client::BroadcastClient as RawBroadcastClient;
/// Generated tonic service trait and server wrapper for the broadcast API.
pub use broadcast::broadcast_server::{Broadcast, BroadcastServer};
/// Generated protobuf request, response, and enum types.
pub use broadcast::*;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
/// Windows live implementation of the generated [`Broadcast`] service.
pub use broadcast_service::BroadcastService;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
/// Builder for configuring a Windows live [`BroadcastService`].
pub use broadcast_service::BroadcastServiceBuilder;

/// Protobuf file descriptor set for reflection services.
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("iracing.broadcast");

/// Generated protobuf and tonic bindings for `iracing.broadcast`.
pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}
