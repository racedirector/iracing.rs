# Broadcast command development

These authored documents explain command intent and development boundaries.
The [clap tree](../../src/commands.rs) and [top-level composition](../../src/lib.rs)
own the executable inventory and exact syntax. Use runtime help for arguments;
these pages are not generated help or a second protocol specification.

Browse the command groups: [camera](camera/), [replay](replay/), [pit](pit/),
[chat](chat/), [textures](textures/), [telemetry](telemetry/), [ffb](ffb/), and
[video](video/). Each executable leaf declaration links to its own document.
When adding or changing a leaf, update that document and declaration link in the
same change. Rustdoc links use repository URLs under cfg(doc), preserving normal
clap help text. Grouping variants and flattened containers are not executable leaves.

## Execution contract

All leaves dispatch a typed operation through [BroadcastCommands](../../src/lib.rs).
Direct leaves never acquire session metadata. Replay session-time lookup also
calls ReplaySessions, resolves a command-owned session view, and suppresses
broadcast when acquisition or resolution fails. The
[session selector](../../src/session_select.rs) owns matching and time parsing.

The [standalone application](../../src/application.rs) lazily initializes its
broadcast and live-session resources independently. It requires connected
Windows shared memory for session lookup and does not wait for metadata. The
[SDK CLI application](../../../iracing-sdk-cli/src/application/mod.rs) embeds
this tree with a separate resource/readiness policy; do not infer one executable's
wait behavior from the other. Embedders can inject portable fakes through the
same capability traits; that does not make production Win32 sending portable.

A successful send is transport success, not simulator acknowledgment. Leaves
return Result<()> and do not serialize result data to stdout. The executables
propagate failures to their entry points; logging and diagnostic presentation
belong there. The simulator owns effects such as UI changes, file creation,
service availability, and replay position. No leaf verifies those effects.

## Ownership and verification

The [SDK](../../../iracing-broadcast-sdk/src/command.rs) owns typed operation
encoding and validation; [iracing-irsdk](../../../iracing-irsdk/src/lib.rs) owns
wire/domain values. Preserve those owner links instead of copying protocol tables.

[CLI tests](../../src/lib.rs) cover parser composition and injected dispatch;
[application tests](../../src/application.rs) cover lazy resource independence.
[SDK tests](../../../iracing-broadcast-sdk/src/command.rs) exercise representative
encodings and validation. Per-command pages identify relevant checks and coverage
limits. These tests do not run simulator UI or prove simulator acceptance.

For a documentation change, run the repository gates and this crate's doctests,
Rustdoc with warnings denied, and bin checks. Build development Rustdoc with
`--document-private-items` to render the leaf enums in the private command module.
When touching the SDK CLI delegation link, also run its bin/doc checks. The SDK
CLI is binary-only and has no library doctest target. Follow root AGENTS and
[development guidance](../../../../docs/development.md).
