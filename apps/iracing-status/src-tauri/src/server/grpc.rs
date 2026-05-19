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
use tonic_health::ServingStatus;

#[cfg(windows)]
type GrpcRoutes = tonic::service::Routes;

pub(super) fn start_grpc_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    tracing::debug!(settings = ?settings, "starting gRPC server");
    start_grpc_server_platform(settings)
}

#[cfg(windows)]
fn start_grpc_server_platform(settings: TransportSettings) -> Result<ServerHandle, String> {
    tracing::debug!("building gRPC services");
    let services = GrpcServices::build()?;
    start_listener_transport(settings, "gRPC", "http", move |listener, shutdown| {
        run_grpc_server(listener, services, shutdown);
    })
}

#[cfg(not(windows))]
fn start_grpc_server_platform(_settings: TransportSettings) -> Result<ServerHandle, String> {
    tracing::debug!("gRPC server start rejected; broadcast server requires Windows");
    Err("gRPC broadcast server requires Windows.".to_string())
}

#[cfg(windows)]
struct GrpcServices {
    routes: tonic::service::RoutesBuilder,
    reflection_descriptor_sets: Vec<&'static [u8]>,
    health_reporter: tonic_health::server::HealthReporter,
}

#[cfg(windows)]
impl GrpcServices {
    fn build() -> Result<Self, String> {
        let (health_reporter, health_service) = tonic_health::server::health_reporter();
        let mut services = Self {
            routes: tonic::service::Routes::builder(),
            reflection_descriptor_sets: vec![tonic_health::pb::FILE_DESCRIPTOR_SET],
            health_reporter,
        };

        services.add_health_service(health_service);
        services.add_broadcast_service()?;
        services.add_telemetry_service()?;
        services.add_reflection_service()?;

        tracing::debug!("gRPC services built");
        Ok(services)
    }

    fn add_health_service(
        &mut self,
        health_service: tonic_health::pb::health_server::HealthServer<
            impl tonic_health::pb::health_server::Health,
        >,
    ) {
        tracing::debug!("adding gRPC health service");
        self.routes.add_service(health_service);
    }

    fn add_broadcast_service(&mut self) -> Result<(), String> {
        tracing::debug!("adding gRPC broadcast service");
        let broadcast = BroadcastService::new()
            .map_err(|error| format!("gRPC failed to initialize broadcast service: {error}"))?;
        self.routes.add_service(BroadcastServer::new(broadcast));
        self.reflection_descriptor_sets
            .push(BROADCAST_FILE_DESCRIPTOR_SET);

        Ok(())
    }

    fn add_telemetry_service(&mut self) -> Result<(), String> {
        tracing::debug!("gRPC telemetry service is not implemented");

        Ok(())
    }

    fn add_reflection_service(&mut self) -> Result<(), String> {
        tracing::debug!("adding gRPC reflection services");
        let reflection_v1 = self
            .reflection_builder()
            .build_v1()
            .map_err(|error| format!("gRPC failed to initialize v1 reflection service: {error}"))?;
        self.routes.add_service(reflection_v1);

        let reflection_v1alpha = self.reflection_builder().build_v1alpha().map_err(|error| {
            format!("gRPC failed to initialize v1alpha reflection service: {error}")
        })?;
        self.routes.add_service(reflection_v1alpha);

        Ok(())
    }

    fn reflection_builder(&self) -> tonic_reflection::server::Builder<'static> {
        let mut builder = tonic_reflection::server::Builder::configure();
        for descriptor_set in &self.reflection_descriptor_sets {
            builder = builder.register_encoded_file_descriptor_set(descriptor_set);
        }
        builder
    }

    fn into_routes(self) -> GrpcRoutes {
        self.routes.routes()
    }

    async fn set_serving(&self) {
        self.health_reporter
            .set_service_status("", ServingStatus::Serving)
            .await;
        self.health_reporter
            .set_serving::<BroadcastServer<BroadcastService>>()
            .await;
    }
}

#[cfg(windows)]
fn run_grpc_server(listener: TcpListener, services: GrpcServices, shutdown: Arc<AtomicBool>) {
    let endpoint = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_else(|error| format!("<unknown: {error}>"));
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        tracing::debug!(
            endpoint = %endpoint,
            "failed to build gRPC tokio runtime"
        );
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            tracing::debug!(
                endpoint = %endpoint,
                "failed to convert gRPC listener to tokio listener"
            );
            return;
        };
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let mut builder = tonic::transport::Server::builder();

        services.set_serving().await;
        let health_reporter = services.health_reporter.clone();
        tracing::debug!(endpoint = %endpoint, "gRPC server accepting requests");
        let result = builder
            .add_routes(services.into_routes())
            .serve_with_incoming_shutdown(incoming, async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
                health_reporter
                    .set_service_status("", ServingStatus::NotServing)
                    .await;
                health_reporter
                    .set_not_serving::<BroadcastServer<BroadcastService>>()
                    .await;
            })
            .await;

        match result {
            Ok(()) => tracing::debug!(endpoint = %endpoint, "gRPC server exited"),
            Err(error) => tracing::debug!(
                endpoint = %endpoint,
                error = %error,
                "gRPC server exited with error"
            ),
        }
    });
}
