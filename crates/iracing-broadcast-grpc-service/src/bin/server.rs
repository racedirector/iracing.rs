use iracing_broadcast_grpc_service::{BroadcastServer, BroadcastService};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let broadcast = BroadcastService::new()?;

    tonic::transport::Server::builder()
        .add_service(BroadcastServer::new(broadcast))
        .serve(addr)
        .await?;

    Ok(())
}
