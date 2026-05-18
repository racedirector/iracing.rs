use super::{settings::TransportSettings, transport::ServerHandle};

#[cfg(windows)]
use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[cfg(windows)]
use super::transport::{start_listener_transport, ACCEPT_POLL_INTERVAL};
#[cfg(windows)]
use iracing_broadcast_grpc_service::{
    BroadcastServer, BroadcastService, FILE_DESCRIPTOR_SET as BROADCAST_FILE_DESCRIPTOR_SET,
};

#[cfg(windows)]
type GrpcRoutes = tonic::service::Routes;

pub(super) fn start_grpc_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    start_grpc_server_platform(settings)
}

#[cfg(windows)]
fn start_grpc_server_platform(settings: TransportSettings) -> Result<ServerHandle, String> {
    let services = GrpcServices::build()?;
    start_listener_transport(settings, "gRPC", "http", move |listener, shutdown| {
        run_grpc_server(listener, services, shutdown);
    })
}

#[cfg(not(windows))]
fn start_grpc_server_platform(_settings: TransportSettings) -> Result<ServerHandle, String> {
    Err("gRPC broadcast server requires Windows.".to_string())
}

#[cfg(windows)]
struct GrpcServices {
    routes: tonic::service::RoutesBuilder,
    reflection_descriptor_sets: Vec<&'static [u8]>,
}

#[cfg(windows)]
impl GrpcServices {
    fn build() -> Result<Self, String> {
        let mut services = Self {
            routes: tonic::service::Routes::builder(),
            reflection_descriptor_sets: Vec::new(),
        };

        services.add_broadcast_service()?;
        services.add_telemetry_service()?;
        services.add_reflection_service()?;

        Ok(services)
    }

    fn add_broadcast_service(&mut self) -> Result<(), String> {
        let broadcast = BroadcastService::new()
            .map_err(|error| format!("gRPC failed to initialize broadcast service: {error}"))?;
        self.routes.add_service(BroadcastServer::new(broadcast));
        self.reflection_descriptor_sets
            .push(BROADCAST_FILE_DESCRIPTOR_SET);

        Ok(())
    }

    fn add_telemetry_service(&mut self) -> Result<(), String> {
        tracing::debug!("TODO: Add the telemetry service");

        Ok(())
    }

    fn add_reflection_service(&mut self) -> Result<(), String> {
        let mut builder = tonic_reflection::server::Builder::configure();
        for descriptor_set in &self.reflection_descriptor_sets {
            builder = builder.register_encoded_file_descriptor_set(descriptor_set);
        }

        let reflection = builder
            .build_v1()
            .map_err(|error| format!("gRPC failed to initialize reflection service: {error}"))?;
        self.routes.add_service(reflection);

        Ok(())
    }

    fn into_routes(self) -> GrpcRoutes {
        self.routes.routes()
    }
}

#[cfg(windows)]
fn run_grpc_server(listener: TcpListener, services: GrpcServices, shutdown: Arc<AtomicBool>) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let mut builder = tonic::transport::Server::builder();

        let _ = builder
            .add_routes(services.into_routes())
            .serve_with_incoming_shutdown(incoming, async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
            })
            .await;
    });
}
