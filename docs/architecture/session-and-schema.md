# Session and Schema Model

The SDK handles two related schemas:

- the variable schema describes the binary layout of each telemetry frame;
- the session schema describes iRacing's YAML metadata.

They travel together at runtime but have different sources, validation, and
generation paths.

## Variable schema

`IbtLayout` in `types/ibt` derives validated metadata and frame regions from a
decoded `Header` and the complete source length. Its private `ParsedIbtHeader`
validates signed fields before geometry is constructed. `MetadataRegions` lives
alongside the other region value objects in `types/regions`; frame counts derive
from physical bytes, not the disk sub-header record count. The layout performs
no I/O, schema decoding, or playback management. `IbtReader` uses it as the sole
authority for source geometry. Fresh snapshot methods read the advertised regions
on each call and return `None` for absent regions. The reader retains no schema
or session cache. `IbtProvider` builds its schema once with
`TelemetryLayout::try_from_headers(&headers, frame_size)`. Missing variable metadata
is accepted only when there are no telemetry frames. Shared validation lives in
`parse_utils` and uses ordinary functions.

Disk variable headers are parsed from `.ibt` files. Live variable headers are
discovered from Windows shared memory. Both become `TelemetryLayout` containing
ordered `FieldLayout` entries and the authoritative frame size.

Consumers should resolve fields through the schema and decode through `VarData`
or `TelemetryValue`. This keeps type sizes, arrays, bitfields, bounds, and
little-endian conversion centralized.

The live header/variable discovery modules are Windows-gated. The resulting
schema and frame types are platform-neutral.

`FieldLayout::data_type()` uses `irsdk::VariableType`, also re-exported at the
crate root. Only the six SDK storage kinds are valid telemetry metadata;
header conversion, layout validation, and runtime decoding reject the count
sentinel. Array strides use SDK byte widths. `VarData` supports `u8` for
characters, `bool`, `i32`, `BitField`, `f32`, `f64`, and their vectors.

Metadata serialization emits `Character`, `Boolean`, `Integer`, `BitField`,
`Float`, and `Double`. Deserialization also accepts the previous `Char`, `Bool`,
`Int32`, `Float32`, and `Float64` names. The former synthetic integer storage
variants are no longer supported. `TelemetryValue` retains its payload names
and variants for source/serialization compatibility, but SDK decoding only
produces the supported storage values and arrays. `DynamicFrame::bitfield`
replaces the former synthetic unsigned-integer shortcut.

## Runtime layout foundation

`TelemetryLayout::try_from_headers(&VariableHeaders, frame_size)` builds an owned
frame description independently of physical `IbtLayout`. Each `FieldLayout`
stores its name, SDK `VariableType`, descriptive `FieldMetadata`, and one
`VariableRegion`; offset and count are not duplicated. The copyable region is
bounded by the advertised frame size at conversion time.

The canonical field array preserves exact header publication order. `FieldId`
is its layout-local index; `fields()` exposes that order and `field_by_name()`
resolves through a name index. IDs must not be transferred between different
layouts. Construction rejects empty/duplicate names, nonpositive frame size or
element count, negative offsets, arithmetic overflow, and out-of-frame extents.
It does not sort fields, reject in-frame overlaps, or normalize names. An empty
header snapshot is valid with a positive frame size; providers decide whether
their source requires metadata.

Providers, connections, `FramePacket`, and `DynamicFrame` retain one shared
`Arc<TelemetryLayout>`. `LayoutProvider` exposes it and supports field inspection.
The legacy `VariableInfo`, `VariableSchema`, and `SchemaProvider` are removed.
`FramePacket::new` returns an error for short or oversized buffers; its bytes and
layout are private so this invariant cannot be invalidated after construction.

`VarData::decode_field` validates storage/shape, then slices the complete region
once. Arrays traverse relative exact chunks with explicit little-endian decoding.
`TelemetryValue::decode_field` uses the same boundary. Generated adapters resolve
names, types, and scalar/array shape once and retain declaration-ordered `FieldId`
slots. Calculated expressions reference slots as well. Adaptation checks shared
layout identity; IDs cannot silently select fields from a different layout.
The runtime layout has no serde or reference-schema compatibility contract.
Historical variable capture DTOs stay local to export/benchmark tooling.

The repository's [IBT specification](../ibt-spec.md) describes positive counts,
six storage widths, and bounded extents. Unknown/sentinel discriminants are
already rejected by wire decoding into `VariableType`. The older public SDK
[type definitions](https://github.com/vipoo/irsdk/blob/master/irsdk_defines.h)
and [memory server](https://github.com/vipoo/irsdk/blob/master/irsdk_server/irsdk_memserver.cpp)
corroborate those widths and append fields at the current buffer end using
`element_width * count`. That observed producer order is not a new validation
rule. The configured sibling `irsdk_1_20` source was unavailable when this
foundation was implemented; verification against that exact version remains
a review prerequisite before closing the foundation issue.

## Session YAML ownership

The [session module Rustdoc](../../crates/iracing-sdk/src/schema/session/mod.rs)
owns the decoding/cleanup/deserialization boundary map. Its owning stages are:

- [Session snapshots and payload decoding](../../crates/iracing-sdk/src/types/session_info_bytes.rs).
- [Internal text sanitization](../../crates/iracing-sdk/src/types/iracing_session_string.rs).
- [Typed deserialization and tests](../../crates/iracing-sdk/src/schema/session/mod.rs).
- [Acquisition and parsed-provider composition](../../crates/iracing-sdk/src/provider.rs).

Consult those owners for byte encoding, cleanup, absence, and error contracts.
Parsing does not own runtime version tracking or publication.

## Caching and publication

The telemetry task has source-specific session policies:

- live: detect version transitions, immediately own the current YAML, and parse
  queued snapshots sequentially on a background FIFO worker before publishing;
- IBT: fetch and parse immutable session YAML once before frames.

Session version tracking and publication belong to these policies, not to
`SessionInfo::parse`.

## Typed session model

The [session types](../../crates/iracing-sdk/src/schema/session/mod.rs) and their
serde attributes own known fields, required/defaulted sections, and optional
unknown-field retention. [Discovery helpers](../../crates/iracing-sdk/src/schema/session/discovery.rs)
own reporting; [parsing tests](../../crates/iracing-sdk/src/schema/session/tests.rs)
exercise those boundaries. The [CLI session command source](../../crates/iracing-sdk-cli/src/commands/session/mod.rs)
owns operator tools, rather than this architecture page.

## Reference provenance

The [reference guide](../reference/README.md) explains how to use type-derived
schemas and retained simulator captures. Its [manifest](../reference/generation.toml)
owns YAML provenance and inventory; the [reference tooling](../../xtask/src/generated.rs)
owns generation and verification. These artifacts provide model output or
historical evidence, rather than an authored runtime architecture specification.
Session contracts belong to the typed source model and its parsing tests.

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


Metadata acquisition traits and their absence/error behavior are documented in
[provider.rs](../../crates/iracing-sdk/src/provider.rs). Variable metadata and
runtime schema contracts remain separate from session text parsing.
