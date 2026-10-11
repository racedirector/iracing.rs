# pit clear

Request clearing the simulator's pending pit-service selections.

## Input and mapping

This parameterless operation maps to the SDK Clear mode. It is broader than the individual tire, fuel, tearoff, or fast-repair clear leaves; do not silently replace it with several broadcasts or add a current-service query.

The [command conversion](../../../src/commands.rs) produces
`Pit(PitCommand::Clear)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The PitCommand conversion owns this one-to-one mapping; shared command-definition and dispatch tests cover composition. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
