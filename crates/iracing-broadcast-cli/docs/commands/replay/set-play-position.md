# replay set-play-position

Request a replay playhead position using a domain origin mode and a frame argument. This is a simulator replay command, not indexed access to an IbtFile.

## Input and mapping

The parser maps the origin to ReplayPositionMode, while clap bounds the frame to u32. The CLI does not validate the frame against replay length. Keep protocol word splitting in the SDK rather than splitting the frame in command dispatch.

The [command conversion](../../../src/commands.rs) produces
`ReplaySetPlayPosition(mode, frame)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

CLI test `replay_position_parses_domain_mode` owns mode parsing; SDK test `replay_position_splits_frame_number_into_words` owns full-width frame encoding. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
