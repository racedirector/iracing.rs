use crate::{
    app::{AppState, build_router},
    config::ServerConfig,
};
use anyhow::{Context, Result, anyhow};
use std::future::IntoFuture;
use tokio::{net::TcpListener, time};

pub(crate) async fn serve(config: ServerConfig) -> Result<()> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind telemetry server to {}", config.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read telemetry server local address")?;
    let shutdown_timeout = config.shutdown_timeout;
    let environment = config.environment;
    let app = build_router(AppState::new(config));

    tracing::info!(
        %local_addr,
        %environment,
        "telemetry HTTP/WebSocket server listening",
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .into_future();

    tokio::pin!(server);

    tokio::select! {
        result = server.as_mut() => {
            result.context("telemetry HTTP/WebSocket server failed")?;
        }
        () = shutdown_signal() => {
            let _ = shutdown_tx.send(());
            time::timeout(shutdown_timeout, server.as_mut())
                .await
                .map_err(|_| anyhow!("telemetry HTTP/WebSocket server did not stop within {:?}", shutdown_timeout))?
                .context("telemetry HTTP/WebSocket server failed during graceful shutdown")?;
        }
    }

    tracing::info!("telemetry HTTP/WebSocket server stopped");
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install Ctrl-C handler");
        return;
    }

    tracing::info!("shutdown signal received");
}
