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
pub enum GetRootResponse {
    /// HTTP server banner.
    Status200_HTTPServerBanner
    (String)
    ,
    /// No HTTP endpoint is registered for the requested path.
    Status404_NoHTTPEndpointIsRegisteredForTheRequestedPath
    (String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum GetSchemaResponse {
    /// The OpenAPI schema for this HTTP server.
    Status200_TheOpenAPISchemaForThisHTTPServer
    (String)
    ,
    /// No HTTP endpoint is registered for the requested path.
    Status404_NoHTTPEndpointIsRegisteredForTheRequestedPath
    (String)
}




/// Meta
#[async_trait]
#[allow(clippy::ptr_arg)]
pub trait Meta<E: std::fmt::Debug + Send + Sync + 'static = ()>: super::ErrorHandler<E> {
    /// Service banner.
    ///
    /// GetRoot - GET /
    async fn get_root(
    &self,
    
    method: &Method,
    host: &Host,
    cookies: &CookieJar,
    ) -> Result<GetRootResponse, E>;

    /// OpenAPI schema.
    ///
    /// GetSchema - GET /schema
    async fn get_schema(
    &self,
    
    method: &Method,
    host: &Host,
    cookies: &CookieJar,
    ) -> Result<GetSchemaResponse, E>;
}
