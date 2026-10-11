# Reference data

This directory retains type-derived schemas and historical simulator captures.
The [YAML provenance manifest](generation.toml) records their source, production
command, and known capture context. It owns the YAML inventory; use it rather
than a separate list of outputs.

Do not hand-edit generated or captured contents to match documentation.

## Production and verification

The [reference tooling](../../xtask/src/generated.rs) owns deterministic schema
generation and verification. Run `cargo xtask generate-reference` from the
workspace root when updating type-derived output, review the diff, then run
`cargo xtask check-generated`. The [repository task](../../xtask/src/main.rs)
composes this check into `check-repo`; the [quality workflow](../../.github/workflows/quality.yml)
owns automated execution.

The typed [session model](../../crates/iracing-sdk/src/schema/session/mod.rs)
owns session fields. The CLI's [session schema command](../../crates/iracing-sdk-cli/src/commands/session/schema.rs)
produces schemas from acquired session values; it does not reproduce a historical
capture without the original source and capture context.

The manifest distinguishes reproducible workspace output from captures whose
raw inputs or generator/build revisions are unavailable. Captured YAML receives
structural verification, not a reproducibility guarantee. The disk capture's
filename supplies a historical date, while live capture dates remain unknown.
Consult the manifest for those limits before drawing conclusions from a sample.

The retained [`formula_ford.json`](formula_ford.json) is a telemetry-value sample,
not a schema or an output of the YAML reference checker. It was added in
[commit 2693a1e](https://github.com/racedirector/iracing.rs/commit/2693a1e309cbe8a561e440c15b1171e614e20558)
on August 6, 2026. Its capture date, simulator build, source recording, and export
command are unknown; the addition date does not establish the capture date.
Preserve it as historical evidence rather than a current runtime contract.

Deterministic IBT fixtures have a separate owner in the
[test-fixtures crate](../../crates/test-fixtures); reference generation does not
regenerate those recordings.

## Using reference data in development

Use captured variable metadata and session examples as evidence when choosing
real names, types, counts, units, and representative values for tests or
benchmarks. Captures do not establish an exhaustive field set across cars,
tracks, sessions, or simulator versions. Current telemetry contracts belong to
[TelemetryLayout](../../crates/iracing-sdk/src/types/telemetry_layout.rs), and wire
storage kinds and enum/flag values belong to [iracing-irsdk](../../crates/iracing-irsdk/src/lib.rs).

Keep each captured layout coherent: offsets, frame size, types, and counts belong
to the same snapshot. When deriving a smaller synthetic input, preserve its
selected fields' real metadata and validate its geometry through the owning
layout implementation. A schema generated from Rust types describes the typed
model; it does not prove that every simulator session has that shape.
