use std::sync::Arc;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use headers::Host;
use http::Method;
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
    start_axum_transport(settings, "HTTP", "http", move |_shutdown| {
        iracing_status_http_api::server::new(Arc::new(StatusHttpApi::new(Arc::clone(
            &observer,
        ))))
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
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetHealthResponse, ()> {
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
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetRootResponse, ()> {
        Ok(GetRootResponse::Status200_HTTPServerBanner(
            "iRacing status HTTP server\n".to_string(),
        ))
    }

    async fn get_schema(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetSchemaResponse, ()> {
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
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
    ) -> Result<GetStatusResponse, ()> {
        Ok(GetStatusResponse::Status200_TheCurrentConnectionStateSnapshot(
            map_connection_state(self.observer.current_state()),
        ))
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

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}
