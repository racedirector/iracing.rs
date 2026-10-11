# Simulation status navigation

The [iracing-simulation crate Rustdoc](../../crates/iracing-simulation/src/lib.rs)
owns the boundary between status probes and caller lifecycle policy.

- [HTTP probe and client contracts](../../crates/iracing-simulation/src/simulation.rs)
  describe status interpretation, errors, injection, and transport limitations.
- [Windows process probe](../../crates/iracing-simulation/src/process.rs) describes
  process matching, platform assumptions, and handle ownership.
- Tests beside those implementations own the executable status and matching
  checks.
- [Crate examples](../../crates/iracing-simulation/examples/) and the workspace
  [lifecycle monitor](../../examples/iracing-lifecycle-monitor/) demonstrate
  consumer composition.

See the [crate README](../../crates/iracing-simulation/README.md) for usage and
local development commands.
