# Simulation Status Architecture

`iracing-simulation` answers a deliberately narrow question: does the iRacing
status endpoint currently report a running simulation? It also exposes an
independent Windows process check.

## HTTP facade

`Simulation<C = StdSimStatusClient>` owns host, port, timeout, and a client.
Construction supports:

- `Simulation::local()` for the default `127.0.0.1:32034`;
- `Simulation::new` for a different endpoint with the default client;
- `Simulation::new_with_client` for dependency injection.

`sim_status_url` is the single formatter for
`/get_sim_status?object=simStatus`.

## Client seam

`SimStatusClient` returns only a status code and body. The high-level facade
deliberately collapses client errors to `false`; it returns `true` only for a 2xx
response whose body contains `running:1`.

This small interface lets examples and applications adapt `reqwest`, `ureq`, or
another stack without forcing an HTTP runtime into the default library path.
Unit tests use fake clients rather than live sockets.

## Default client

`StdSimStatusClient` uses `TcpStream`, explicit connect/read/write timeouts, a
minimal HTTP/1.1 GET request, and a small response parser. That parser recognizes
the numeric status from the first line and treats bytes after the first header
terminator as the body.

It is intentionally not a general HTTP implementation. Redirects, TLS,
chunked-body decoding, and rich transport errors belong in an injected client if
needed.

## Process detection

On Windows, `process.rs` snapshots the process list with ToolHelp APIs and
performs case-insensitive exact executable-name matching for
`iRacingSim64DX11.exe`.

Process presence and HTTP status are separate signals:

- `check_sim_status` does not fall back to process detection;
- process detection does not prove the simulation endpoint is ready;
- the lifecycle-monitor example composes both with live SDK connectivity.

Keep pure matching/decoding helpers testable independently from Win32 calls.
