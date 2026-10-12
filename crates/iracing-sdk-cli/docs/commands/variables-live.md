# variables live

Inspect published variable descriptions without decoding frame values or generating a variable schema.

The live leaf is declared only on Windows. The executable supplies its [application](../../src/application/mod.rs); production requires its live adapter to initialize successfully against available simulator telemetry. The capability owns readiness and acquisition errors. Consult [live adapter source](../../src/application/live.rs) for current lifecycle behavior.

## Execution and side effects

Consume the owned variable-header snapshot from the injected `LiveVariables` capability. The command does not read individual telemetry fields or invoke broadcast.

Acquire the header collection before constructing the writer. Acquisition errors therefore leave an existing output file untouched; an empty snapshot is serialized as an empty collection. Successful execution finalizes the writer and reports flush errors.
The command performs no simulator mutation.

## Output and edge cases

An empty collection is a successful metadata result, not an acquisition failure. Serialization and flush failures propagate after output has opened.

The owning [VariableHeader](../../../iracing-irsdk/src/variable_header.rs) and [VariableHeaders](../../../iracing-sdk/src/types/variable_headers.rs) define value representation and interpretation.
[DocumentWriter](../../src/writer/document.rs) owns selected serialization,
newline/framing behavior, and flushing; [OutputSink](../../src/writer/output_sink.rs)
owns file/stdout I/O. Clap owns exact options and accepted format names.
Output failures can leave a prefix written; success does not imply filesystem durability.

## Development references

[Command implementation and tests](../../src/commands/variables/mod.rs) own execution order.
The adjacent Windows fake-capability test checks dispatch and empty collection JSON output.
The live branch accepts its injected snapshot capability; the IBT branch opens
the recording directly. Preserve the command's stated empty/missing-data and
error behavior, and inspect output opening order when changing validation.
