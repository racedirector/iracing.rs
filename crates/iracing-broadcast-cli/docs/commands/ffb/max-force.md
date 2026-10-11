# ffb max-force

Request a force-feedback maximum-force value using the CLI's nm input.

## Input and mapping

Clap parses an f32 and the CLI forwards it unchanged. There is no positivity, finiteness, device-range, or simulator-state check. SDK conversion owns the floating-point representation; do not silently round, clamp, or add unit conversion in dispatch.

The [command conversion](../../../src/commands.rs) produces
`ForceFeedback(nm)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

SDK test `force_feedback_preserves_existing_encoding` owns representation preservation. The declaration is the current input validation boundary. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
