#[cfg(windows)]
use iracing_broadcast_grpc_service::{BroadcastServer, BroadcastService};

#[cfg(windows)]
use tonic_health::ServingStatus;

#[cfg(windows)]
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,iracing_broadcast_grpc_service=debug,iracing_sdk=info")
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

    let addr = std::env::var("BROADCAST_ADDR")
        .unwrap_or_else(|_| "[::1]:50051".to_string())
        .parse()?;
    tracing::info!(%addr, "starting broadcast gRPC server");
    let broadcast = BroadcastService::new()?;
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("", ServingStatus::Serving)
        .await;
    health_reporter
        .set_serving::<BroadcastServer<BroadcastService>>()
        .await;

    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(BroadcastServer::new(broadcast))
        .serve(addr)
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
