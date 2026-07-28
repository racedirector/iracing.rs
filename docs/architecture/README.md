# Architecture Guide

This directory is the durable design map for the `iracing.rs` workspace. It
documents boundaries and behavior that are otherwise spread across manifests,
module trees, CI workflows, and examples.

These documents describe the implementation in the repository, including known
incomplete transitions. They are not a wish list. When code and documentation
disagree, verify the implementation and update both in the same change.

## Document map

- [Workspace structure](workspace.md): crate responsibilities, dependency
  direction, runtime paths, and placement rules.
- [Telemetry pipeline](telemetry-pipeline.md): `.ibt` and live acquisition,
  providers, task policies, connections, frames, and typed adapters.
- [Session and schema model](session-and-schema.md): variable metadata, YAML
  cleanup/parsing, session caching, schema discovery, and generated references.
- [Platform and feature boundaries](platform-and-features.md): Windows, portable,
  and WASM surfaces plus Cargo feature intent.
- [Broadcast service](broadcast-service.md): the gRPC service's layered
  command-and-observation architecture and operational boundary.
- [Simulation status](simulation-status.md): the small HTTP facade, injection
  seam, and Windows process detection.
- [Testing and fixtures](testing-and-fixtures.md): deterministic data,
  test-helper contracts, and CI-aligned validation.

The broadcast crate also has a detailed local document at
[`crates/iracing-broadcast-grpc-service/docs/architecture.md`](../../crates/iracing-broadcast-grpc-service/docs/architecture.md).
The repo-level document explains how that crate fits into the workspace; the
crate-local document is authoritative for RPC behavior and extension work.

## Reading paths

For telemetry work, read this index, [workspace structure](workspace.md), and
[telemetry pipeline](telemetry-pipeline.md). Add
[session and schema model](session-and-schema.md) when changing YAML, types, or
schema tools.

For Windows or release changes, also read
[platform and feature boundaries](platform-and-features.md).

For test failures involving recordings, read
[testing and fixtures](testing-and-fixtures.md) before changing fixture paths or
adding skip behavior.

## Documentation contract

Architecture changes should update the closest document in this directory and
the affected crate's `AGENTS.md`. Keep the levels distinct:

- `AGENTS.md` holds commands, constraints, and short warnings needed during an
  editing session.
- Architecture documents explain why boundaries exist and how components
  interact.
- Rustdoc explains public API behavior and invariants.
- README files teach consumers how to start using a crate or example.
- `docs/reference/*.yml` is generated schema output and must not be hand-edited.

Prefer implementation links and stable type/module names over line numbers.
Record observable semantics—ownership, ordering, loss, retries, cancellation,
and platform availability—not incidental control flow.

When auditing these documents:

1. Inspect `Cargo.toml`, `.cargo/config.toml`, and `.github/workflows`.
2. Inspect public exports and cfg/feature gates in each crate.
3. Trace at least one real request or frame end to end.
4. Run link/path checks and the relevant Cargo gates.
5. Call out an incomplete migration explicitly instead of documenting its
   intended end state as already complete.
