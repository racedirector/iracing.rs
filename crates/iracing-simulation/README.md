# iracing-simulation

Independent helpers for observing iRacing HTTP status and Windows process presence.
Start with the [crate documentation](src/lib.rs) for the ownership boundary.

## Quick start

```rust,no_run
use iracing_simulation::Simulation;

let simulation = Simulation::local();
println!("running={}", simulation.check_sim_status());
```

For response semantics, client injection, errors, and transport limitations, see
[the HTTP probe implementation and Rustdoc](src/simulation.rs). Windows process
checks have their own [implementation and Rustdoc](src/process.rs).

## Examples

The [examples directory](examples/) demonstrates polling and alternative HTTP
clients. From the workspace root, inspect a representative example's options:

```sh
cargo run -p iracing-simulation --example check-connection -- --help
```

Applications compose these probes with their own lifecycle policy. The workspace
[lifecycle monitor](../../examples/iracing-lifecycle-monitor/) shows one consumer.

## Development

Run from the workspace root:

```sh
cargo test -p iracing-simulation --all-targets
cargo test -p iracing-simulation --doc
cargo check -p iracing-simulation --examples --bins
```

The unit tests beside each implementation own the executable status and matching
checks. See [repository development guidance](../../docs/development.md) for the
broader validation procedure.
