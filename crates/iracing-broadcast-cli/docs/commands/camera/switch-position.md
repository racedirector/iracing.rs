# camera switch-position

Select a camera using the simulator's position selector, camera group, and camera index. This changes the viewing target; it does not query standings or translate a position into a car index.

## Input and mapping

The three selectors remain u16 values. The default camera index is supplied by clap; conversion preserves selectors without checking that the current replay contains them. Do not add a session lookup merely to dispatch this direct operation.

The [command conversion](../../../src/commands.rs) produces
`CameraSwitchPosition(position, group, camera)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

SDK test `camera_switch_position_encodes_to_broadcast_message` checks selector encoding. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
