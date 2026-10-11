# video stop

Request ending the simulator's video capture.

## Input and mapping

This maps to EndVideoCapture, not a process shutdown or CLI recorder cancellation. The CLI does not wait for file flushing or inspect a capture result. Simulator-side completion is outside the command response.

The [command conversion](../../../src/commands.rs) produces
`VideoCapture(VideoCaptureMode::EndVideoCapture)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The VideoCommand conversion and SDK video encoding own the mapping; shared error propagation tests own send failures. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
