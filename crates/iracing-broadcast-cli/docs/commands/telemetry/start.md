# telemetry start

Request that the simulator start its disk telemetry recording.

## Input and mapping

This is a broadcast control operation, distinct from the SDK CLI's telemetry record exporter. It does not choose a filename, open an IBT, or return recording bytes. The simulator owns recording availability and destination.

The [command conversion](../../../src/commands.rs) produces
`Telemetry(TelemetryCommandMode::Start)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The TelemetryCommand conversion owns the Start mapping; shared dispatch tests own transport error propagation. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
