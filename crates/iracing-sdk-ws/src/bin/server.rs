use std::{error::Error, net::SocketAddr};

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(address).await?;

    axum::serve(listener, iracing_sdk_ws::router()).await?;

    Ok(())
}
