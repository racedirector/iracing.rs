# AGENTS.md

Guidance for agents editing `crates/iracing-simulation`.

## Validation

- `cargo test -p iracing-simulation --all-targets` runs unit and target tests.
- `cargo test -p iracing-simulation --doc` runs doctests.
- Build docs with `RUSTDOCFLAGS=-D warnings` using
  `cargo doc -p iracing-simulation --no-deps`.
- `cargo check -p iracing-simulation --examples --bins` checks sample applications.
- Follow the root AGENTS and [development procedure](../../docs/development.md)
  for repository gates.

## Navigation and testing

- Start at [crate Rustdoc](src/lib.rs); HTTP contracts and tests live in
  [simulation.rs](src/simulation.rs), Windows process contracts and tests in
  [process.rs](src/process.rs).
- Prefer injected fake clients over live sockets for status-policy tests.
- When changing transport parsing, cover realistic status lines, header
  termination, and malformed responses beside the parser.
- Keep pure process matching/decoding tests independent of Windows enumeration.
- Examples demonstrate client adapters and polling; inspect their clap options
  with `cargo run -p iracing-simulation --example <name> -- --help`.
- Use real endpoint smoke tests only when the local simulator is available.
