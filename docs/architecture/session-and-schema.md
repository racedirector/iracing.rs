# Session and Schema Model

The SDK handles two related schemas:

- the variable schema describes the binary layout of each telemetry frame;
- the session schema describes iRacing's YAML metadata.

They travel together at runtime but have different sources, validation, and
generation paths.

## Variable schema

Disk variable headers are copied as `VariableHeadersBuffer` snapshots by the
IBT reader, then interpreted by `IbtProvider`. Live variable headers are copied
from Windows shared memory. Both become `VariableSchema` containing named
`VariableInfo` entries and a frame size outside the byte-acquisition layer.

Consumers should resolve fields through the schema and decode through `VarData`
or `TelemetryValue`. This keeps type sizes, arrays, bitfields, bounds, and
little-endian conversion centralized.

The shared live/IBT header representation is platform-neutral. Windows-gated
live code reads that header and discovers variable definitions from shared
memory; the resulting schema and frame types remain platform-neutral.

## Session YAML path

iRacing session data can contain control characters, non-UTF-8 bytes, and YAML
that standard parsers do not accept directly. The path has three explicit steps:

- readers copy the advertised region into an owned `SessionInfoBuffer`;
- converting that buffer to `String` stops at the first NUL and preserves
  invalid UTF-8 bytes with a single-byte fallback;
- `yaml_utils::preprocess_iracing_yaml` repairs problematic control characters
  and unquoted fields before `SessionInfo::parse` deserializes the typed model.

Provider and caller contracts must make preprocessing ownership clear; do not
stack ad hoc cleaners at each call site.

## Caching and publication

The telemetry task has source-specific session policies:

- live: detect version transitions, immediately own the current YAML, and parse
  queued snapshots sequentially on a background FIFO worker before publishing;
- IBT: fetch and parse immutable session YAML once before frames.

Session parsing itself is stateless. Version tracking, ordering, retry, and
publication remain telemetry-policy responsibilities.

## Typed session model

`types/session` decomposes `SessionInfo` into weekend, timing, driver, radio,
camera, car setup, and session-data modules. Serde uses iRacing's PascalCase
field naming.

The `schema-discovery` feature adds flattened maps for unknown YAML fields and
helpers that collect their paths, inferred types, and examples. This supports
evolving the typed model without silently losing evidence of new simulator
fields.

## Generated reference artifacts

`docs/reference/*.yml` contains checked-in output from schema binaries:

- baseline session, variable, and primitive schemas;
- disk-derived session and variable schemas;
- live-derived variable and session schemas.

These files are generated artifacts. Change the Rust model or generator first,
then regenerate via the Cargo aliases in `.cargo/config.toml`. Most schema
binaries require `codegen,schema-discovery`; live discovery also requires
Windows and an appropriate simulator state.

`docs/reference/README.md` is the usage index for those artifacts.

## Change rules

- New telemetry primitive behavior belongs in SDK types/decoders, not generated
  YAML snapshots.
- New known session fields require typed serde model changes and appropriate
  schema regeneration.
- Unknown-field discovery should remain opt-in because it changes serialized
  model shape and dependencies.
- Bounds and encoding checks belong before deserialization.
- Session version, retry, ordering, and EOF behavior belong in the telemetry
  session policies, not individual consumers.
