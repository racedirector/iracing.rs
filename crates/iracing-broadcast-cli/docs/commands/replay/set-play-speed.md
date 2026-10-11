# replay set-play-speed

Request replay speed and the slow-motion interpretation together. The signed speed is a command input, not a measurement of current playback.

## Input and mapping

The CLI stores speed as i16 and forwards the slow-motion boolean unchanged. It does not clamp speed, inspect replay length, or certify that the simulator will honor a value. Preserve signed inputs and use SDK conversion for encoding.

The [command conversion](../../../src/commands.rs) produces
`ReplaySetPlaySpeed(speed, slow_motion)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The CLI direct-replay conversion and `replay_group_keeps_direct_subcommands_alongside_resolved_search` are the relevant dispatch seams. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
