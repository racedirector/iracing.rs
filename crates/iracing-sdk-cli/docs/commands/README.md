# SDK CLI command development

Clap declarations in [commands](../../src/commands/mod.rs) and
[SourceKind](../../src/utils.rs) own executable vocabulary and platform gates.
These authored documents explain command development contracts, not generated help.

| Command family | IBT leaf | Windows live leaf |
| --- | --- | --- |
| Headers | [headers ibt](headers-ibt.md) | [headers live](headers-live.md) |
| Variables | [variables ibt](variables-ibt.md) | [variables live](variables-live.md) |
| Session snapshot | [session snapshot ibt](session-snapshot-ibt.md) | [session snapshot live](session-snapshot-live.md) |
| Session discovery | [session discover ibt](session-discover-ibt.md) | [session discover live](session-discover-live.md) |
| Session schema | [session schema ibt](session-schema-ibt.md) | [session schema live](session-schema-live.md) |

[Broadcast](broadcast.md) delegates to the owning broadcast CLI.
Telemetry command documents await the prepared projection work tracked by
[#254](https://github.com/racedirector/iracing.rs/issues/254); this index does not
specify that evolving implementation.

The [application](../../src/application/mod.rs) owns resource construction and
implements the [injected capabilities](../../src/commands/dependencies.rs).
Live documents describe the value consumed by a command, while the
[live adapter](../../src/application/live.rs) owns acquisition and lifecycle.
[DocumentWriter](../../src/writer/document.rs) and
[OutputSink](../../src/writer/output_sink.rs) own formatting and destination I/O.
Commands own when output is opened; writes are not transactional.

When changing a command, inspect its run method and adjacent tests before
updating its document. Keep schema shapes, wire representation, live safety,
and exact argument syntax at their owning source/tests. Run repository gates
and warnings-denied private CLI Rustdoc; compile CLI binaries on supported
platforms so Windows-only declarations and capability tests remain checked.
