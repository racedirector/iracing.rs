# IncidentFlags Int32 Compatibility

## Summary

Make `IncidentFlags` accept the live-schema `Int32` representation in addition to `BitField`, so derived adapters, dynamic/schema-driven reads, and direct `VarData` decoding all work without a special-case workaround in user code. Keep every other bitflags wrapper strict.

## Key Changes

- `crates/iracing-sdk/src/types/irsdk/flags.rs` defines the canonical `IncidentFlags` type and accepts either `VariableType::BitField` or `VariableType::Int32` in `VarData::from_bytes`.
- Preserve the raw bit pattern when decoding `Int32`, including high-bit values, so packed-field and structured accessors work on every valid incident payload.
- Expose `report()`, `penalty()`, and `classify()` for structured access while retaining `report_bits()` and `penalty_bits()` for raw packed-field access.
- Preserve the incident-specific JSON Schema annotations for masks and named report/penalty codes.
- Leave the rest of the bitflag wrappers unchanged.
- Do not add a separate derive-macro fallback branch unless tests show a gap after the `VarData` fix. The derive path already validates and decodes through `VarData`, so this should unblock it automatically.
- Optionally clean up the `examples/driver-inputs/src/driver_input.rs` workaround comment once the type path works end to end.

## Test Plan

- Unit-test `IncidentFlags::from_bytes` with `VariableType::Int32` using a `PlayerIncidents`-style payload.
- Keep a regression test that the same type decodes from `VariableType::BitField`.
- Verify high-bit/sign preservation so the `Int32` representation does not lose raw flags when its signed value is negative.
- Keep adapter-level coverage proving `telemetry_type_mismatch_details::<IncidentFlags>` accepts both schema representations and that a derived frame field can validate and adapt either representation.
- If the example is updated, build or test the `examples/driver-inputs` target to confirm the field compiles without the commented calculated fallback.

## Assumptions

- `IncidentFlags` is the only mixed-type wrapper that needs this dual decoding path.
- Existing `x-irsdk-unit-ref: '#/$defs/IncidentFlags'` annotations stay as-is; no schema regeneration is required for this fix.
- The intended compatibility behavior is “accept both, preserve raw bits,” not “normalize the schema to BitField.”
