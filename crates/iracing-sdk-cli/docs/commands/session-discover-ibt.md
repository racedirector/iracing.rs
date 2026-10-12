# session discover ibt

Report fields retained as unknown by the current session model to support schema development.

The IBT leaf is portable and requires a readable completed recording accepted by `IbtFile::open`. Source validation belongs to [IbtFile](../../../iracing-sdk/src/ibt/file.rs), not the CLI.

## Execution and side effects

Open the recording through `IbtFile::open` and use [parse_ibt_session](../../src/commands/session/mod.rs). For an available decoded session, call `collect_unknown_fields`; do not independently scan or parse the original YAML.

Finish source acquisition, parsing, and unknown-field collection before constructing the writer. An absent session yields an empty list and still opens output successfully. Successful execution finalizes the writer and reports flush errors.
The command performs no simulator mutation.

## Output and edge cases

Absence and an available session with no unknown fields both yield an empty list. Acquisition or parsing failures propagate and must not be converted into an empty result. Discovery neither modifies the model nor rewrites generated reference artifacts.

The owning [SessionInfo and unknown-field collection](../../../iracing-sdk/src/schema/session/mod.rs) defines value representation and interpretation.
[DocumentWriter](../../src/writer/document.rs) owns selected serialization,
newline/framing behavior, and flushing; [OutputSink](../../src/writer/output_sink.rs)
owns file/stdout I/O. Clap owns exact options and accepted format names.
Output failures can leave a prefix written; success does not imply filesystem durability.

## Development references

[Command implementation and tests](../../src/commands/session/discover.rs) own execution order.
Consult the owning session parsing/discovery tests for unknown-field retention; command dispatch is in the run method.
The live branch accepts its injected snapshot capability; the IBT branch opens
the recording directly. Preserve the command's stated empty/missing-data and
error behavior, and inspect output opening order when changing validation.
