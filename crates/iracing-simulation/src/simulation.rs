//! Synchronous status probing, with transport separated from the running predicate.
//!
//! [`Simulation`] owns endpoint configuration and delegates each request to its
//! client. [`SimStatusClient`] owns transport errors; the facade reduces them to
//! a boolean. Tests inject responses so this policy does not depend on a live
//! endpoint. Client adapters own any additional caching or retry behavior.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use thiserror::Error;

/// Default host used by iRacing's HTTP status endpoint.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// Default port used by iRacing's HTTP status endpoint.
pub const DEFAULT_PORT: u16 = 32034;

/// HTTP path (including query string) for the iRacing sim-status endpoint.
pub const SIM_STATUS_PATH: &str = "/get_sim_status?object=simStatus";

/// Build the full URL for the iRacing sim-status endpoint.
///
/// Custom [`SimStatusClient`] implementations should use this instead of
/// hardcoding the path so they stay in sync if it ever changes.
///
/// ```rust
/// use iracing_simulation::{sim_status_url, DEFAULT_HOST, DEFAULT_PORT};
///
/// let url = sim_status_url(DEFAULT_HOST, DEFAULT_PORT);
/// assert_eq!(url, "http://127.0.0.1:32034/get_sim_status?object=simStatus");
/// ```
pub fn sim_status_url(host: &str, port: u16) -> String {
    format!("http://{}:{}{}", host, port, SIM_STATUS_PATH)
}

/// Minimal response surface area needed by Simulation.
/// Consumers can implement `SimStatusClient` using any HTTP library,
/// caching layer, or custom transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimStatusResponse {
    /// HTTP status code returned by the server.
    pub status_code: u16,
    /// Response body as a UTF-8 string.
    pub body: String,
}

/// Errors returned when fetching the iRacing sim-status endpoint fails.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimStatusError {
    /// The configured host and port could not be parsed as a socket address.
    #[error("invalid simulation status address `{address}`")]
    InvalidAddress {
        /// Address string that failed to parse.
        address: String,
    },
    /// The client could not connect to the sim-status endpoint.
    #[error("failed to connect to simulation status endpoint at {address}: {message}")]
    Connect {
        /// Endpoint address that could not be reached.
        address: String,
        /// Underlying transport error text.
        message: String,
    },
    /// The client could not configure the socket timeout.
    #[error("failed to configure simulation status socket timeout: {message}")]
    ConfigureTimeout {
        /// Underlying socket configuration error text.
        message: String,
    },
    /// The client could not write the HTTP request to the socket.
    #[error("failed to send simulation status request to {address}: {message}")]
    RequestWrite {
        /// Endpoint address being queried.
        address: String,
        /// Underlying write error text.
        message: String,
    },
    /// Reading the response exceeded the configured timeout.
    #[error("timed out reading simulation status response after {timeout_ms}ms")]
    ReadTimeout {
        /// Configured timeout in milliseconds.
        timeout_ms: u128,
    },
    /// The client could not read the HTTP response from the socket.
    #[error("failed to read simulation status response from {address}: {message}")]
    ResponseRead {
        /// Endpoint address being queried.
        address: String,
        /// Underlying read error text.
        message: String,
    },
    /// The endpoint returned bytes that were not parseable as the expected HTTP response.
    #[error("simulation status response was not valid HTTP")]
    InvalidResponse,
    /// Error reported by a custom [`SimStatusClient`] implementation.
    #[error("simulation status request failed: {message}")]
    Client {
        /// Custom client error text.
        message: String,
    },
}

/// Synchronous HTTP transport seam for [`Simulation`].
///
/// Implementations return the status and body without deciding whether the
/// simulator is running. The facade owns that predicate. Implementations may
/// supply caching, retries, or another HTTP stack; the facade invokes this method
/// once per check.
pub trait SimStatusClient {
    /// Send a GET request to the iRacing sim-status endpoint and return the response.
    ///
    /// Implementors should connect to `host:port`, issue the request within
    /// `timeout`, and map transport or protocol errors to [`SimStatusError`].
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, SimStatusError>;
}

/// Built-in HTTP/1.1 client using [`TcpStream`] without an HTTP-library dependency.
///
/// The host and port must parse as a numeric [`SocketAddr`]; this client does not
/// resolve DNS names. It sends a GET with `Connection: close`, reads until EOF,
/// and requires UTF-8 response bytes with a header terminator and numeric status
/// code. It does not follow redirects, use TLS, or decode chunked bodies.
///
/// The requested timeout is applied separately to connection, socket reads, and
/// socket writes. An elapsed-time guard also bounds the read loop between reads;
/// this is not a single deadline covering all request phases. Transport and
/// parsing failures are reported as [`SimStatusError`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StdSimStatusClient;

impl SimStatusClient for StdSimStatusClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, SimStatusError> {
        let address = format!("{host}:{port}");
        let addr: SocketAddr = address
            .parse()
            .map_err(|_| SimStatusError::InvalidAddress {
                address: address.clone(),
            })?;

        let mut stream =
            TcpStream::connect_timeout(&addr, timeout).map_err(|err| SimStatusError::Connect {
                address: address.clone(),
                message: err.to_string(),
            })?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|err| SimStatusError::ConfigureTimeout {
                message: err.to_string(),
            })?;
        stream.set_write_timeout(Some(timeout)).map_err(|err| {
            SimStatusError::ConfigureTimeout {
                message: err.to_string(),
            }
        })?;

        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Connection: close\r\n\
             Accept: */*\r\n\
             \r\n",
            path = SIM_STATUS_PATH,
            host = host,
            port = port
        );

        stream
            .write_all(request.as_bytes())
            .map_err(|err| SimStatusError::RequestWrite {
                address: address.clone(),
                message: err.to_string(),
            })?;

        // Read the full response until EOF. Timeouts are enforced by the socket;
        // keep a secondary guard to bound total loop time for odd cases.
        let start = Instant::now();
        let mut bytes = Vec::<u8>::new();
        let mut chunk = [0u8; 8192];

        loop {
            if start.elapsed() > timeout {
                return Err(SimStatusError::ReadTimeout {
                    timeout_ms: timeout.as_millis(),
                });
            }

            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                Err(err) => {
                    return Err(SimStatusError::ResponseRead {
                        address: address.clone(),
                        message: err.to_string(),
                    });
                }
            }
        }

        parse_http_response(&bytes).ok_or(SimStatusError::InvalidResponse)
    }
}

/// Configured HTTP status probe.
///
/// Construction stores host, port, timeout, and client without connecting.
/// [`Self::check_sim_status`] requests status on demand; this type owns no polling
/// task or reconnect state. The default client is [`StdSimStatusClient`].
pub struct Simulation<C: SimStatusClient = StdSimStatusClient> {
    host: String,
    port: u16,
    timeout: Duration,
    client: C,
}

impl Simulation<StdSimStatusClient> {
    /// Configure a probe for the local iRacing instance using the default client.
    ///
    /// This is the right choice for the common case: iRacing running on the
    /// same machine. Uses [`DEFAULT_HOST`] and [`DEFAULT_PORT`].
    pub fn local() -> Self {
        Self::new(DEFAULT_HOST, DEFAULT_PORT)
    }

    /// Configure a probe at `host:port` using the default client.
    ///
    /// Use this when the sim is running on a remote machine or a non-standard
    /// port. For the local case, prefer [`Simulation::local`].
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::new_with_client(host, port, StdSimStatusClient)
    }
}

impl<C: SimStatusClient> Simulation<C> {
    /// Create a `Simulation` using a custom [`SimStatusClient`].
    ///
    /// Use this when you need to inject a caching layer, retry logic, or an
    /// alternate HTTP stack. For the common case, prefer [`Simulation::new`].
    /// The initial request timeout is five seconds.
    pub fn new_with_client(host: impl Into<String>, port: u16, client: C) -> Self {
        Self {
            host: host.into(),
            port,
            timeout: Duration::from_millis(5000),
            client,
        }
    }

    /// Set the timeout passed to the client on subsequent checks.
    ///
    /// The client determines how to enforce it; see [`StdSimStatusClient`] for
    /// the built-in transport's timeout behavior.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Request status once and return whether the endpoint reports running.
    ///
    /// Returns `true` exactly when the response status is in `200..=299` and the
    /// body contains the literal substring `running:1`. This is a substring
    /// predicate, not structured response parsing. Any client error or other
    /// response returns `false`; callers needing error details can use the
    /// [`SimStatusClient`] directly.
    ///
    /// This check does not fall back to Windows process detection or establish
    /// telemetry connectivity.
    pub fn check_sim_status(&self) -> bool {
        let resp = match self
            .client
            .get_sim_status(&self.host, self.port, self.timeout)
        {
            Ok(r) => r,
            Err(_) => return false,
        };

        (200..=299).contains(&resp.status_code) && resp.body.contains("running:1")
    }
}

/// Extremely small HTTP parser:
/// - extracts status code from the status line
/// - treats everything after the first `\r\n\r\n` as body
///
/// Notes:
/// - This parser serves the built-in status transport, not general HTTP clients.
/// - It does not implement chunked decoding; if the server uses chunked encoding,
///   the body will include the chunk framing.
fn parse_http_response(bytes: &[u8]) -> Option<SimStatusResponse> {
    let s = std::str::from_utf8(bytes).ok()?;
    let header_end = s.find("\r\n\r\n")?;

    let (head, body_with_sep) = s.split_at(header_end);
    let body = &body_with_sep["\r\n\r\n".len()..];

    let status_line = head.lines().next()?;
    // e.g. "HTTP/1.1 200 OK"
    let mut parts = status_line.split_whitespace();
    let _http_ver = parts.next()?;
    let status_code = parts.next()?.parse::<u16>().ok()?;

    Some(SimStatusResponse {
        status_code,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeClient {
        resp: Result<SimStatusResponse, SimStatusError>,
    }

    impl SimStatusClient for FakeClient {
        fn get_sim_status(
            &self,
            _host: &str,
            _port: u16,
            _timeout: Duration,
        ) -> Result<SimStatusResponse, SimStatusError> {
            self.resp.clone()
        }
    }

    #[test]
    fn check_sim_status_true_when_2xx_and_running_1() {
        let sim = Simulation::new_with_client(
            "127.0.0.1",
            32034,
            FakeClient {
                resp: Ok(SimStatusResponse {
                    status_code: 200,
                    body: "simStatus running:1".into(),
                }),
            },
        );

        assert!(sim.check_sim_status());
    }

    #[test]
    fn check_sim_status_false_when_non_2xx() {
        let sim = Simulation::new_with_client(
            "127.0.0.1",
            32034,
            FakeClient {
                resp: Ok(SimStatusResponse {
                    status_code: 500,
                    body: "simStatus running:1".into(),
                }),
            },
        );

        assert!(!sim.check_sim_status());
    }

    #[test]
    fn check_sim_status_false_when_missing_marker() {
        let sim = Simulation::new_with_client(
            "127.0.0.1",
            32034,
            FakeClient {
                resp: Ok(SimStatusResponse {
                    status_code: 200,
                    body: "simStatus running:0".into(),
                }),
            },
        );

        assert!(!sim.check_sim_status());
    }

    #[test]
    fn check_sim_status_false_on_client_error() {
        let sim = Simulation::new_with_client(
            "127.0.0.1",
            32034,
            FakeClient {
                resp: Err(SimStatusError::Client {
                    message: "sim unavailable".into(),
                }),
            },
        );

        assert!(!sim.check_sim_status());
    }

    #[test]
    fn std_client_parses_basic_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nsimStatus running:1";
        let parsed = parse_http_response(raw).unwrap();
        assert_eq!(parsed.status_code, 200);
        assert!(parsed.body.contains("running:1"));
    }
}
