# replay search

Move the replay using an SDK search mode, such as stepping between boundaries. The simulator performs the search; the CLI does not scan a recording.

## Input and mapping

The [domain parser](../../../src/parser.rs) restricts input to supported spellings and maps to ReplaySearchMode. That parser owns exact vocabulary. No live-session resolution is needed, even for modes referring to sessions.

The [command conversion](../../../src/commands.rs) produces
`ReplaySearch(mode)`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Uses BroadcastCommands without calling ReplaySessions or acquiring telemetry. Production sending requires the Windows broadcast adapter; simulator readiness and whether the operation takes effect are not checked by this leaf.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

CLI test `replay_search_parses_domain_mode` checks the domain mapping. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
