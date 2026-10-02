# Schema reference

JSON schema snapshots retained as reference data. Session schema generation is available through the [`iracing-sdk` CLI](../../crates/iracing-sdk-cli/src/main.rs); static session, variable and primitive reference generation is maintained by `cargo xtask generate-reference`. Car-setup/live-variable CLI generators remain deferred.

Do not hand-edit.

## Using these artifacts

Use this directory as the source of truth before creating telemetry schemas,
session YAML, or binary frames for tests, fixtures, benchmarks, and simulated
providers:

- `variable-schema.yml` defines the shape of one `VariableInfo`, including the
  supported data types and the meaning of `offset`, `count`, and
  `count_as_time`.
- `disk-variable-schema.yml` and `live-variable-schema.yml` contain generated
  examples with real iRacing variable names, types, array counts, units,
  descriptions, offsets, and frame sizes. Disk recordings and live shared
  memory expose different sets and layouts.
- `primitives-schema.yml` defines the available iRacing enum and bitflag value
  domains referenced by variable units such as `irsdk_Flags` and
  `irsdk_TrkSurf`.
- `session-schema.yml` describes the typed `SessionInfo` contract;
  `live-session-schema.yml` records a concrete discovered session shape and
  example, including concrete nested keys and representative values.

Treat generated disk/live layouts as coherent snapshots, not mix-and-match
field catalogs: if a synthetic frame reuses offsets, its buffer must use the
same snapshot's `frame_size`, types, and counts. The observed snapshots are
representative rather than a guarantee that every car, track, session type, or
iRacing build exposes exactly the same fields. Prefer deriving a smaller test
schema from a snapshot when a full captured layout is unnecessary, while
retaining the selected fields' real names and metadata.

| Artifact | Purpose | Regenerate from workspace root |
| --- | --- | --- |
| [`session-schema.yml`](session-schema.yml) | Baseline schema for `iracing_sdk::schema::SessionInfo`. | `cargo iracing-sdk session schema type --output ./docs/reference/session-schema.yml` |
| [`variable-schema.yml`](variable-schema.yml) | Baseline schema for `iracing_sdk::VariableInfo`. | `cargo xtask generate-reference` (static schemas only). |
| [`primitives-schema.yml`](primitives-schema.yml) | `$defs` bank for `irsdk_*` primitive wrappers (enums/bitflags). | `cargo xtask generate-reference` (static schemas only). |
| [`disk-variable-schema.yml`](disk-variable-schema.yml) | Retained representative disk telemetry schema, captured from `fordmustanggt4_interlagos gp 2026-08-01 11-24-51.ibt`. | Original capture input is absent; see provenance below. |
| [`live-session-schema.yml`](live-session-schema.yml) | Schema generated from live session YAML. Windows-only. | `cargo iracing-sdk session schema live --output ./docs/reference/live-session-schema.yml` |
| [`live-variable-schema.yml`](live-variable-schema.yml) | Schema generated from live telemetry variables. Windows-only. | Live variable generator remains deferred. |

## Generation contract and provenance

`generation.toml` inventories every YAML artifact. `cargo xtask check-generated`
(and `check-repo` / quality CI) generates the three static schemas in a temporary
directory and compares exact bytes without modifying committed outputs. Run
`cargo xtask generate-reference`, review the diff, and rerun the check to update.
Generation serializes through sorted JSON values before YAML, so map order is
stable. Generator/catalog code lives in `xtask/src/generated.rs`; Cargo.lock fixes
the schema library version. Add new native primitive wrappers to the catalog when
extending the documented primitive bank.

The disk-variable Interlagos input is **not committed**; hydrated Donington and
deterministic fixture recordings are different layouts and cannot reproduce it.
It is classified as a capture, not a deterministic artifact. Live captures also
lack committed raw input and known capture/build/generator revisions. The manifest
records these unknowns explicitly. Captures get static schema-envelope/example
validation, not a claim of reproducibility. Preserve their offsets/frame size
coherently. None is exhaustive across cars, tracks, sessions or simulator versions.

The existing `cargo test-fixtures` remains the sole owner of deterministic IBT
fixture regeneration and drift; this checker does not generate another IBT set.
