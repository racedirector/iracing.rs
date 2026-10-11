# camera switch-number

Select the viewed car by its displayed number while choosing a camera group and index. A displayed number is a string, not an SDK car index.

## Input and mapping

Keep the number as text through CLI conversion so leading zeroes reach the SDK's padding logic. The CLI accepts arbitrary text; SDK conversion owns fallback and overflow behavior. Consult that contract before tightening validation, rather than silently normalizing the number.

The [command conversion](../../../src/commands.rs) produces
`CameraSwitchNumber(car_number, group, camera)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

The SDK's [padding implementation](../../../../iracing-broadcast-sdk/src/pad_car_number.rs) owns number encoding edge cases; it has no dedicated tests today. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
