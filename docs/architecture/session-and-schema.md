# Session and Schema Model

The SDK handles two related schemas:

- the variable schema describes the binary layout of each telemetry frame;
- the session schema describes iRacing's YAML metadata.

They travel together at runtime but have different sources, validation, and
generation paths.

## Variable schema

Disk variable headers are parsed from `.ibt` files. Live variable headers are
discovered from Windows shared memory. Both become `VariableSchema` containing
named `VariableInfo` entries and a frame size.

Consumers should resolve fields through the schema and decode through `VarData`
or `TelemetryValue`. This keeps type sizes, arrays, bitfields, bounds, and
little-endian conversion centralized.

The live header/variable discovery modules are Windows-gated. The resulting
schema and frame types are platform-neutral.

## Session YAML path

iRacing session data can contain control characters and non-UTF-8 bytes. The
provider-facing path is implemented by the representations in
`schema/session/types.rs`: sources create bounded `SessionYamlBytes`, providers
decode and sanitize them, and `Provider::session_yaml` returns owned
`SanitizedSessionYaml`. `SessionInfo::parse_sanitized` then performs typed serde
deserialization. `SessionInfo::parse` is the convenience entry point for an
already-decoded string and applies the same sanitation before deserializing.

The public `yaml_utils` module is a legacy compatibility surface with different
decoding behavior. Production providers do not use it. Migrating, deprecating,
or removing that module is a separate public-API decision; tests for the active
provider pipeline must not duplicate a second extraction/decoding matrix there.

## Caching and publication

The telemetry task has source-specific session policies:

- live: detect version transitions, immediately own the current YAML, and parse
  queued snapshots sequentially on a background FIFO worker before publishing;
- IBT: fetch and parse immutable session YAML once before frames.

Architecture changes must distinguish parser caching from telemetry publication.
They solve different problems.

## Typed session model

`schema/session` decomposes `SessionInfo` into weekend, timing, driver, radio,
camera, car setup, and session-data modules. Serde uses iRacing's PascalCase
field naming.

The `schema-discovery` feature adds flattened maps for unknown YAML fields and
helpers that collect their paths, inferred types, and examples. This supports
evolving the typed model without silently losing evidence of new simulator
fields.

## Generated reference artifacts

`docs/reference/*.yml` contains checked-in output from schema binaries:

- baseline session, variable, and primitive schemas;
- disk-derived variable schema;
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
