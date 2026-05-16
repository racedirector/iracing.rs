use iracing_broadcast_grpc_service::{BroadcastServer, BroadcastService};

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
