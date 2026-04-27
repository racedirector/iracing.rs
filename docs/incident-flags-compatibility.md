# IncidentFlags Int32 Compatibility Fix

## Summary
Make `IncidentFlags` accept the live-schema `Int32` representation in addition to `BitField`, so derived adapters, dynamic/schema-driven reads, and direct `VarData` decoding all work without a special-case workaround in user code. Keep every other bitflags wrapper strict.

## Key Changes
- Update `crates/iracing-sdk/src/types/irsdk_bitflags.rs` so `IncidentFlags::from_bytes` accepts either `VariableType::BitField` or `VariableType::Int32`.
- Preserve the raw bit pattern when decoding `Int32`, including high-bit values, so `report_code()` and `penalty_code()` keep working on all valid incident payloads.
- Leave the rest of the bitflag wrappers unchanged.
- Do not add a separate derive-macro fallback branch unless tests show a gap after the `VarData` fix. The derive path already validates and decodes through `VarData`, so this should unblock it automatically.
- Optionally clean up the `examples/driver-inputs/src/driver_input.rs` workaround comment once the type path works end to end.

## Test Plan
- Add a unit test for `IncidentFlags::from_bytes` with `VariableType::Int32` using a `PlayerIncidents`-style payload.
- Add a regression test that the same type still decodes from `VariableType::BitField`.
- Add a high-bit/sign-preservation test so the fallback does not lose raw flags when the `Int32` value is negative.
- Add an adapter-level regression test proving `telemetry_type_mismatch_details::<IncidentFlags>` accepts the `Int32` schema entry and that a derived frame field typed as `IncidentFlags` can validate/adapt against it.
- If the example is updated, build or test the `examples/driver-inputs` target to confirm the field compiles without the commented calculated fallback.

## Assumptions
- `IncidentFlags` is the only mixed-type wrapper that needs this dual decoding path.
- Existing `x-irsdk-unit-ref: '#/$defs/IncidentFlags'` annotations stay as-is; no schema regeneration is required for this fix.
- The intended compatibility behavior is “accept both, preserve raw bits,” not “normalize the schema to BitField.”
