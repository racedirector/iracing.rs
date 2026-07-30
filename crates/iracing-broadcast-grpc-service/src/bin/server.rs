#[cfg(windows)]
use std::net::SocketAddr;

#[cfg(windows)]
use iracing_broadcast_grpc_service::{BroadcastServer, BroadcastService, FILE_DESCRIPTOR_SET};

#[cfg(windows)]
use iracing_sdk::providers::live::LiveProvider;

#[cfg(windows)]
use socket2::{Domain, Protocol, Socket, Type};

#[cfg(windows)]
use tokio::net::TcpListener;

#[cfg(windows)]
use tokio_stream::wrappers::TcpListenerStream;

#[cfg(windows)]
use tonic_health::pb::FILE_DESCRIPTOR_SET as HEALTH_FILE_DESCRIPTOR_SET;

#[cfg(windows)]
use tonic_health::ServingStatus;

#[cfg(windows)]
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,iracing_broadcast_grpc_service=trace,iracing_sdk=debug,tonic=info,tower=info",
        )
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE)
        .init();
}

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let addr: SocketAddr = std::env::var("BROADCAST_ADDR")
        .unwrap_or_else(|_| "[::]:50051".to_string())
        .parse()?;
    tracing::info!(
        %addr,
        default_filter = "info,iracing_broadcast_grpc_service=trace,iracing_sdk=debug,tonic=info,tower=info",
        "starting broadcast gRPC server",
    );
    tracing::debug!("opening live telemetry provider");
    let live_provider = LiveProvider::new()?;
    tracing::debug!("constructing broadcast service with externally provided live telemetry");
    let broadcast = BroadcastService::builder()
        .with_live_provider(live_provider)
        .build()?;
    tracing::info!("broadcast service initialized");
    tracing::debug!("registering health and reflection services");
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    let reflection_v1_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(HEALTH_FILE_DESCRIPTOR_SET)
        .build_v1()?;
    let reflection_v1alpha_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(HEALTH_FILE_DESCRIPTOR_SET)
        .build_v1alpha()?;
    health_reporter
        .set_service_status("", ServingStatus::Serving)
        .await;
    health_reporter
        .set_serving::<BroadcastServer<BroadcastService>>()
        .await;
    tracing::info!(
        reflected_service = "iracing.broadcast.Broadcast",
        reflected_health_service = "grpc.health.v1.Health",
        reflection_versions = "v1,v1alpha",
        "gRPC reflection and health services registered",
    );

    tracing::debug!("creating dual-stack tcp listener");
    let socket = if addr.is_ipv4() {
        Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?
    } else {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_only_v6(false)?;
        socket
    };
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    let listener = TcpListener::from_std(socket.into())?;
    let port = addr.port();
    tracing::info!(
        %addr,
        dual_stack = !addr.is_ipv4(),
        reflection = true,
        health = true,
        reachable_ipv4 = format!("127.0.0.1:{}", port),
        reachable_ipv6 = format!("[::1]:{}", port),
        "listener bound",
    );

    tracing::info!("serving gRPC traffic");
    tonic::transport::Server::builder()
        .add_service(reflection_v1_service)
        .add_service(reflection_v1alpha_service)
        .add_service(health_service)
        .add_service(BroadcastServer::new(broadcast))
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    tracing::info!("broadcast gRPC server stopped");

    Ok(())
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the iRacing broadcast gRPC server requires Windows",
    )
    .into())
}
