# session schema live

Generate a schema from an available decoded session value using `schemars::schema_for_value!`.

The live leaf is declared only on Windows. The executable supplies its [application](../../src/application/mod.rs); production requires its live adapter to initialize successfully against available simulator telemetry. The capability owns readiness and acquisition errors. Consult [live adapter source](../../src/application/live.rs) for current lifecycle behavior.

## Execution and side effects

Consume an optional decoded session from the injected `LiveSessions` capability, then generate the schema from that value. The command does not create a frame subscription or define live lifecycle policy.

Require a session and generate its schema before constructing the writer.
Missing-session, source, and parsing errors occur before output opening;
`schema_for_value!` itself returns a schema rather than a `Result`.
Successful execution finalizes the writer and reports flush errors.
The command performs no simulator mutation.

## Output and edge cases

An absent session is an error, unlike discovery's empty-list result. This invocation emits a document; it does not update captured/generated repository reference artifacts. Schema content follows the owning session model and schemars behavior rather than a hand-maintained CLI field inventory.

The owning [SessionInfo](../../../iracing-sdk/src/schema/session/mod.rs) and the schemars dependency declared in [Cargo.toml](../../Cargo.toml) defines value representation and interpretation.
[DocumentWriter](../../src/writer/document.rs) owns selected serialization,
newline/framing behavior, and flushing; [OutputSink](../../src/writer/output_sink.rs)
owns file/stdout I/O. Clap owns exact options and accepted format names.
Output failures can leave a prefix written; success does not imply filesystem durability.

## Development references

[Command implementation and tests](../../src/commands/session/schema.rs) own execution order.
Consult the owning model/schema tests and reference generation tooling for shape/provenance; this command's run method owns acquisition and output ordering.
The live branch accepts its injected snapshot capability; the IBT branch opens
the recording directly. Preserve the command's stated empty/missing-data and
error behavior, and inspect output opening order when changing validation.
