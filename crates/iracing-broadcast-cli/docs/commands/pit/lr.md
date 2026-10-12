# pit lr

Request the left-rear tire service with the CLI's pressure value.

## Input and mapping

The positional pressure is a u16 in the CLI's psi input and reaches the SDK unchanged. The CLI does not infer a matching front pressure or select another corner. Car-specific pressure limits belong to simulator behavior rather than this parser.

The [command conversion](../../../src/commands.rs) produces
`Pit(PitCommand::LR(psi))`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

Review the PitCommand conversion and SDK pit encoder when changing corner mapping; current pit encoding tests are representative rather than exhaustive. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
