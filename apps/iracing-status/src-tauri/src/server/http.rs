use std::sync::Arc;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use headers::Host;
use http::{Method, Uri};
use iracing_status_http_api::{
    apis::{
        health::{GetHealthResponse, Health as HealthApi},
        meta::{GetRootResponse, GetSchemaResponse, Meta as MetaApi},
        status::{GetStatusResponse, Status as StatusApi},
        ErrorHandler,
    },
    models::{ConnectionStateResponse, ConnectionStatus as HttpConnectionStatus, HealthResponse},
};

use super::{
    settings::TransportSettings,
    transport::{start_axum_transport, ServerHandle},
};
use crate::state::{ConnectionStateObserver, ConnectionStatus, IRacingConnectionState};

const OPENAPI_SCHEMA: &str = include_str!("../../../docs/specs/openapi.yaml");

pub(super) fn start_http_server(
    settings: TransportSettings,
    observer: Arc<ConnectionStateObserver>,
) -> Result<ServerHandle, String> {
    tracing::debug!(settings = ?settings, "starting HTTP server");
    start_axum_transport(settings, "HTTP", "http", move |_shutdown| {
        iracing_status_http_api::server::new(Arc::new(StatusHttpApi::new(Arc::clone(&observer))))
            .fallback(not_found)
    })
}

struct StatusHttpApi {
    observer: Arc<ConnectionStateObserver>,
}

impl StatusHttpApi {
    fn new(observer: Arc<ConnectionStateObserver>) -> Self {
        Self { observer }
    }
}

#[async_trait::async_trait]
impl HealthApi for StatusHttpApi {
    async fn get_health(
        &self,
        method: &Method,
        host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetHealthResponse, ()> {
        tracing::debug!(
            method = %method,
            host = ?host,
            route = "/health",
            "HTTP request"
        );
        Ok(
            GetHealthResponse::Status200_TheHTTPServerIsRunningAndAbleToRespond(
                HealthResponse::new("ok".to_string(), "iracing-status".to_string()),
            ),
        )
    }
}

#[async_trait::async_trait]
impl MetaApi for StatusHttpApi {
    async fn get_root(
        &self,
        method: &Method,
        host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetRootResponse, ()> {
        tracing::debug!(
            method = %method,
            host = ?host,
            route = "/",
            "HTTP request"
        );
        Ok(GetRootResponse::Status200_HTTPServerBanner(
            "iRacing status HTTP server\n".to_string(),
        ))
    }

    async fn get_schema(
        &self,
        method: &Method,
        host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetSchemaResponse, ()> {
        tracing::debug!(
            method = %method,
            host = ?host,
            route = "/schema",
            "HTTP request"
        );
        Ok(
            GetSchemaResponse::Status200_TheOpenAPISchemaForThisHTTPServer(
                OPENAPI_SCHEMA.to_string(),
            ),
        )
    }
}

#[async_trait::async_trait]
impl StatusApi for StatusHttpApi {
    async fn get_status(
        &self,
        method: &Method,
        host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetStatusResponse, ()> {
        let state = self.observer.current_state();
        tracing::debug!(
            method = %method,
            host = ?host,
            route = "/status",
            state = ?state,
            "HTTP request"
        );
        Ok(
            GetStatusResponse::Status200_TheCurrentConnectionStateSnapshot(map_connection_state(
                state,
            )),
        )
    }
}

impl ErrorHandler for StatusHttpApi {}

fn map_connection_state(state: IRacingConnectionState) -> ConnectionStateResponse {
    ConnectionStateResponse::new(
        map_connection_status(state.process),
        map_connection_status(state.sim),
        map_connection_status(state.telemetry),
    )
}

fn map_connection_status(status: ConnectionStatus) -> HttpConnectionStatus {
    match status {
        ConnectionStatus::Disconnected => HttpConnectionStatus::Disconnected,
        ConnectionStatus::Checking => HttpConnectionStatus::Checking,
        ConnectionStatus::Connected => HttpConnectionStatus::Connected,
    }
}

async fn not_found(uri: Uri) -> Response {
    tracing::debug!(route = %uri, "HTTP request not found");
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}
