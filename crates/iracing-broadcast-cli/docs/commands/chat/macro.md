# chat macro

Request sending a configured simulator chat macro by index. The message text comes from simulator configuration rather than the CLI.

## Input and mapping

The parser accepts indices 1 through 15 and the SDK encoder independently enforces that range. The conversion itself merely forwards the index, so programmatic construction must still pass SDK validation. Do not renumber or expand the macro text locally.

The [command conversion](../../../src/commands.rs) produces
`ChatMacro(index)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

SDK tests `chat_macro_encodes_macro_mode_and_number` and `chat_macro_rejects_out_of_range_number` own the encoding range; the clap declaration owns operator validation. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
