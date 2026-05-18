use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
};
use axum_extra::extract::CookieJar;
use headers::Host;
use http::Method;
use iracing_status_http_api::{
    apis::{
        default::{Default as HttpApi, GetHealthResponse, GetRootResponse, GetSchemaResponse},
        ErrorHandler,
    },
    models::HealthResponse,
};

use super::{
    settings::TransportSettings,
    transport::{start_listener_transport, ServerHandle, ACCEPT_POLL_INTERVAL},
};

const OPENAPI_SCHEMA: &str = include_str!("../../../openapi.yaml");

pub(super) fn start_http_server(settings: TransportSettings) -> Result<ServerHandle, String> {
    start_listener_transport(settings, "HTTP", "http", run_http_server)
}

fn run_http_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };

        let _ = axum::serve(listener, router())
            .with_graceful_shutdown(async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
            })
            .await;
    });
}

fn router() -> Router {
    iracing_status_http_api::server::new(Arc::new(StatusHttpApi)).fallback(not_found)
}

struct StatusHttpApi;

#[async_trait::async_trait]
impl HttpApi for StatusHttpApi {
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

impl ErrorHandler for StatusHttpApi {}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found\n").into_response()
}
