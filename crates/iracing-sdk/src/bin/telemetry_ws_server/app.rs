use crate::config::ServerConfig;
use axum::Router;
use tower_http::trace::TraceLayer;

#[derive(Clone, Debug)]
pub(crate) struct AppState {
    pub(crate) config: ServerConfig,
}

impl AppState {
    pub(crate) fn new(config: ServerConfig) -> Self {
        Self { config }
    }
}

pub(crate) fn build_router(state: AppState) -> Router {
    tracing::debug!(
        environment = %state.config.environment,
        "building route-free telemetry router",
    );

    Router::new()
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Environment;
    use std::{net::SocketAddr, time::Duration};
    use tower::ServiceExt;

    #[tokio::test]
    async fn empty_router_returns_not_found() {
        let config = ServerConfig {
            environment: Environment::Test,
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            shutdown_timeout: Duration::from_secs(1),
        };
        let app = build_router(AppState::new(config));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
