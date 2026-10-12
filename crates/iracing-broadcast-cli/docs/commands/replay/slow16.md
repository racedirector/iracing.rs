# replay slow16

Request the predefined slow-motion replay shortcut.

## Input and mapping

Conversion fixes speed to 16 and slow-motion to true. The numeric value is interpreted with the SDK slow-motion flag; do not rewrite it as an ordinary speed multiplier or infer elapsed playback from the command name.

The [command conversion](../../../src/commands.rs) produces
`ReplaySetPlaySpeed(16, true)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The direct-replay conversion owns this shortcut pair; shared direct-replay composition tests are the parser seam. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
