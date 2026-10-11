# replay search-session-time

Resolve an operator-facing session selector against acquired session metadata, then request a replay jump to its time. This leaf acquires metadata before issuing any broadcast.

## Input and mapping

SessionSelector and SessionTime reject malformed syntax and millisecond overflow before execution. Numeric selectors must exist in the acquired list. Name resolution prefers an exact match, then a unique partial match, and rejects ambiguity instead of guessing. The command does not clamp time to session duration or replay availability.

The [command conversion](../../../src/commands.rs) produces
`ReplaySearchSessionTime(resolved.number, time.millis())`. The [SDK operation and encoder](../../../../iracing-broadcast-sdk/src/command.rs)
own typed semantics and wire encoding; [iracing-irsdk](../../../../iracing-irsdk/src/lib.rs)
owns raw domain values. Clap declarations and [domain parsers](../../../src/parser.rs)
own exact syntax, defaults, and accepted spellings.

## Execution and output

Calls ReplaySessions first, then BroadcastCommands only after successful resolution. Both executable applications take an immediate session snapshot without waiting for metadata; absent metadata becomes an empty session list and resolution fails. SDK CLI live initialization additionally validates telemetry layout, while the standalone source requires connected shared memory. Initialization, acquisition, resolution, encoding, and send errors propagate; failed resolution performs no broadcast.

This leaf produces no command-result document or simulator acknowledgment.
The [shared execution contract](../README.md#execution-contract) explains error
reporting, platform constraints, and application resource ownership.

## Changing this command

CLI tests `search_session_time_dispatches_resolved_selector` and `search_session_time_resolution_failure_prevents_dispatch` verify ordering and suppression. [Session-selection tests](../../../src/session_select.rs) own aliases, matching, duration boundaries, and errors. Shared CLI tests in [lib.rs](../../../src/lib.rs) check command definition,
injected dispatch, and propagated errors. Simulator acceptance is not exercised
by those fake-based tests. Change operator parsing at its declaration, conversion
at its match arm, and transport behavior only at the owning adapter.
