use async_trait::async_trait;
use axum::extract::*;
use axum_extra::extract::CookieJar;
use bytes::Bytes;
use headers::Host;
use http::Method;
use serde::{Deserialize, Serialize};

use crate::{models, types::*};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum GetHealthResponse {
    /// The HTTP server is running and able to respond.
    Status200_TheHTTPServerIsRunningAndAbleToRespond
    (models::HealthResponse)
    ,
    /// No HTTP endpoint is registered for the requested path.
    Status404_NoHTTPEndpointIsRegisteredForTheRequestedPath
    (String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum GetRootResponse {
    /// HTTP server banner.
    Status200_HTTPServerBanner
    (String)
    ,
    /// No HTTP endpoint is registered for the requested path.
    Status404_NoHTTPEndpointIsRegisteredForTheRequestedPath
    (String)
}




/// Default
#[async_trait]
#[allow(clippy::ptr_arg)]
pub trait Default<E: std::fmt::Debug + Send + Sync + 'static = ()>: super::ErrorHandler<E> {
    /// Health check.
    ///
    /// GetHealth - GET /health
    async fn get_health(
    &self,
    
    method: &Method,
    host: &Host,
    cookies: &CookieJar,
    ) -> Result<GetHealthResponse, E>;

    /// Service banner.
    ///
    /// GetRoot - GET /
    async fn get_root(
    &self,
    
    method: &Method,
    host: &Host,
    cookies: &CookieJar,
    ) -> Result<GetRootResponse, E>;
}
