use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// Minimal response surface area needed by Simulation.
/// Consumers can implement `SimStatusClient` using any HTTP library,
/// caching layer, or custom transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimStatusResponse {
    pub status_code: u16,
    pub body: String,
}

/// Consumer-injectable HTTP logic.
/// Keep this synchronous + dependency-free from this crate's perspective.
pub trait SimStatusClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, ()>;
}

/// Dependency-free default client: raw HTTP/1.1 over TcpStream.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdSimStatusClient;

impl SimStatusClient for StdSimStatusClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, ()> {
        const PATH: &str = "/get_sim_status?object=simStatus";

        let addr: SocketAddr = format!("{host}:{port}").parse().map_err(|_| ())?;

        let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(|_| ())?;
        stream.set_read_timeout(Some(timeout)).map_err(|_| ())?;
        stream.set_write_timeout(Some(timeout)).map_err(|_| ())?;

        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Connection: close\r\n\
             Accept: */*\r\n\
             \r\n",
            path = PATH,
            host = host,
            port = port
        );

        stream.write_all(request.as_bytes()).map_err(|_| ())?;

        // Read the full response until EOF. Timeouts are enforced by the socket;
        // keep a secondary guard to bound total loop time for odd cases.
        let start = Instant::now();
        let mut bytes = Vec::<u8>::new();
        let mut chunk = [0u8; 8192];

        loop {
            if start.elapsed() > timeout {
                return Err(());
            }

            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                Err(_) => return Err(()),
            }
        }

        parse_http_response(&bytes).ok_or(())
    }
}

/// High-level façade.
/// Default construction uses the dependency-free client, but callers can inject
/// any `SimStatusClient` for caching, retries, alternate HTTP stacks, etc.
pub struct Simulation<C: SimStatusClient = StdSimStatusClient> {
    host: String,
    port: u16,
    timeout: Duration,
    client: C,
}

impl Simulation<StdSimStatusClient> {
    /// Dependency-free default.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::new_with_client(host, port, StdSimStatusClient)
    }
}

impl<C: SimStatusClient> Simulation<C> {
    pub fn new_with_client(host: impl Into<String>, port: u16, client: C) -> Self {
        Self {
            host: host.into(),
            port,
            timeout: Duration::from_millis(5000),
            client,
        }
    }

    /// Optional: allow consumers to tune timeout even with injected clients.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Equivalent semantics to the original JS:
    /// - `false` on any error
    /// - `false` on non-2xx
    /// - `true` only if body contains `running:1`
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
/// - This is sufficient for the local endpoint you showed.
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
        resp: Result<SimStatusResponse, ()>,
    }

    impl SimStatusClient for FakeClient {
        fn get_sim_status(
            &self,
            _host: &str,
            _port: u16,
            _timeout: Duration,
        ) -> Result<SimStatusResponse, ()> {
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
        let sim = Simulation::new_with_client("127.0.0.1", 32034, FakeClient { resp: Err(()) });

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
