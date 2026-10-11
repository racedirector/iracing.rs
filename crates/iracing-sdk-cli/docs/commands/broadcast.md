# Broadcast delegation

The Windows-only broadcast group embeds the
[iracing-broadcast-cli command tree](../../../iracing-broadcast-cli/docs/commands/README.md).
That crate owns executable leaf declarations, operator parsing, conversion,
and command-specific development contracts. This CLI adds no duplicate leaves
or output format for broadcast commands.

The [command dispatcher](../../src/commands/mod.rs) forwards the selected command
to that tree with the SDK CLI's injected application. Direct operations use
BroadcastCommands; replay session-time resolution additionally uses ReplaySessions.
The [application](../../src/application/mod.rs) independently owns its lazy
broadcast and live resources. Its [live adapter](../../src/application/live.rs)
also takes an immediate session snapshot for replay lookup; bounded waits belong
to frame acquisition. Unlike the standalone broadcast session source, SDK CLI
live initialization also validates telemetry layout. Those resource requirements
belong to the executable application rather than the shared command tree.

Broadcast execution returns errors rather than a serialized result document.
Transport success does not acknowledge simulator effects. Exact vocabulary and
input validation remain in the owning clap tree, while SDK operation encoding
stays in iracing-broadcast-sdk. See the owning development tree for side effects
and command-specific edge cases.

When changing delegation, preserve capability injection and resource independence.
Application tests beside the SDK CLI implementation check lazy initialization,
retry after initialization failure, resource reuse, and independent slots.
Broadcast CLI tests check parsed dispatch and resolution failure before sending.
Do not duplicate those contracts as a separate leaf inventory here.
