# replay erase

Request erasure of the simulator replay tape. Treat this as a destructive simulator-side operation, rather than file deletion by the CLI.

## Input and mapping

This leaf has no arguments and performs no confirmation or replay-state lookup. It maps directly to the erase state. Adding confirmation would change behavior and needs separate product approval; documentation must not imply that such protection exists.

The [command conversion](../../../src/commands.rs) produces
`ReplaySetState(ReplayStateMode::EraseTape)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The direct-replay conversion is the operation mapping; shared command-definition and injected-dispatch tests cover the composition boundary. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
