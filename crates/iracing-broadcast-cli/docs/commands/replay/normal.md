# replay normal

Request normal replay playback through the existing playback shortcut.

## Input and mapping

Conversion fixes speed to 1 and slow-motion to false. There is no speed argument, state read, or attempt to restore a remembered prior speed. Keep this mapping distinct from the slow-motion shortcut.

The [command conversion](../../../src/commands.rs) produces
`ReplaySetPlaySpeed(1, false)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The direct-replay conversion owns the shortcut pair. The direct-replay composition test exercises pause and erase as representative leaves, not this normal-speed mapping. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
