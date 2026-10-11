# camera set-state

Request the supplied camera-state bitfield. Named flags are combined into one state; this is not a sequence of individual flag toggles.

## Input and mapping

The argument group requires exactly one representation: raw bits or named flags. Raw input is limited to 16 bits, matching the SDK broadcast argument; unknown in-range raw flags are retained. Repeated named flags are unioned, so duplicates have no additional effect. Conversion does not read or merge the simulator's current state.

The [command conversion](../../../src/commands.rs) produces
`CameraSetState(args.into())`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

CLI tests `camera_state_accepts_raw_bits`, `camera_state_accepts_repeated_named_flags`, `camera_state_rejects_raw_bits_with_named_flags`, and `camera_state_requires_raw_bits_or_named_flags` own representation rules; SDK rejection coverage owns the wire bound. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
