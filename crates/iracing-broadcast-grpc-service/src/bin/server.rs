#[cfg(windows)]
use iracing_broadcast_grpc_service::{BroadcastServer, BroadcastService};

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("BROADCAST_ADDR")
        .unwrap_or_else(|_| "[::1]:50051".to_string())
        .parse()?;
    let broadcast = BroadcastService::new()?;

    tonic::transport::Server::builder()
        .add_service(BroadcastServer::new(broadcast))
        .serve(addr)
        .await?;

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
