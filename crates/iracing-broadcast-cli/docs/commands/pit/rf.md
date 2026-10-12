# pit rf

Request the right-front tire service with the CLI's pressure value.

## Input and mapping

The positional pressure is a u16 in the CLI's psi input and reaches the SDK unchanged. This leaf selects the right-front corner; keep it distinct from left-front even when pressure values coincide. No car-state acquisition runs.

The [command conversion](../../../src/commands.rs) produces
`Pit(PitCommand::RF(psi))`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The PitCommand conversion is the corner mapping; SDK source owns mode/value encoding. Existing pit tests do not assert every tire corner. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
