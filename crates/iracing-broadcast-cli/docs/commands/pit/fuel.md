# pit fuel

Request a pit fuel quantity in the CLI's gallons input.

## Input and mapping

The quantity is a u16 and is forwarded unchanged. The CLI performs no unit conversion, tank-capacity lookup, or car-specific range check; zero remains an accepted numeric value. SDK encoding owns the fuel argument.

The [command conversion](../../../src/commands.rs) produces
`Pit(PitCommand::Fuel(gallons))`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

SDK test `pit_fuel_encodes_mode_and_value` checks mode and unchanged quantity. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
