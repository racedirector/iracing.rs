# pit clear-tires

Request clearing pending tire service selections.

## Input and mapping

This parameterless operation maps to ClearTires, not the broader Clear mode. The command sends one SDK operation without reading existing tire selections. Do not implement clearing by synthesizing four pressure requests.

The [command conversion](../../../src/commands.rs) produces
`Pit(PitCommand::ClearTires)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The PitCommand conversion and SDK pit encoder own the distinct clear-tires mapping; shared dispatch tests own error propagation. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
