# session schema ibt

Generate a schema from an available decoded session value using `schemars::schema_for_value!`.

The IBT leaf is portable and requires a readable completed recording accepted by `IbtFile::open`. Source validation belongs to [IbtFile](../../../iracing-sdk/src/ibt/file.rs), not the CLI.

## Execution and side effects

Open the recording through `IbtFile::open` and use [parse_ibt_session](../../src/commands/session/mod.rs). Generate the schema from the resulting session value; the CLI does not author a parallel session type or schema contract.

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
