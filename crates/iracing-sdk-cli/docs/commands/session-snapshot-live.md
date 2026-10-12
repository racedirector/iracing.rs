# session snapshot live

Capture the decoded session model as a document, rather than copying the original source text.

The live leaf is declared only on Windows. The executable supplies its [application](../../src/application/mod.rs); production requires its live adapter to initialize successfully against available simulator telemetry. The capability owns readiness and acquisition errors. Consult [live adapter source](../../src/application/live.rs) for current lifecycle behavior.

## Execution and side effects

Consume the decoded optional session from the injected `LiveSessions` capability. The command consumes this snapshot; it does not wait for a telemetry frame or implement reconnection.

Require an available decoded session before constructing the writer. Missing session, source errors, and parsing errors leave an existing output file untouched. Successful execution finalizes the writer and reports flush errors.
The command performs no simulator mutation.

## Output and edge cases

Session absence is an error. Serializer output reflects the current model, not original YAML formatting or source byte fidelity. Serialization and final flush errors propagate after output opening.

The owning [SessionInfo](../../../iracing-sdk/src/schema/session/mod.rs) defines value representation and interpretation.
[DocumentWriter](../../src/writer/document.rs) owns selected serialization,
newline/framing behavior, and flushing; [OutputSink](../../src/writer/output_sink.rs)
owns file/stdout I/O. Clap owns exact options and accepted format names.
Output failures can leave a prefix written; success does not imply filesystem durability.

## Development references

[Command implementation and tests](../../src/commands/session/snapshot.rs) own execution order.
The adjacent Windows fake-capability test checks dispatch and the missing-session error.
The live branch accepts its injected snapshot capability; the IBT branch opens
the recording directly. Preserve the command's stated empty/missing-data and
error behavior, and inspect output opening order when changing validation.
