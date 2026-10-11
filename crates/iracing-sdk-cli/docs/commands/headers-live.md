# headers live

Inspect the source's native header values without interpreting them as telemetry fields.

The live leaf is declared only on Windows. The executable supplies its [application](../../src/application/mod.rs); production requires its live adapter to initialize successfully against available simulator telemetry. The capability owns readiness and acquisition errors. Consult [live adapter source](../../src/application/live.rs) for current lifecycle behavior.

## Execution and side effects

Consume one owned header returned by the injected `LiveHeaders` capability, then write that single value. No broadcast capability is invoked.

Construct the writer before opening or acquiring the source. A file destination is created or truncated even when source initialization or acquisition subsequently fails. Successful execution finalizes the writer and reports flush errors.
The command performs no simulator mutation.

## Output and edge cases

The IBT and live output shapes differ deliberately. Source errors, serialization failures, and final flush failures propagate; partially written output is not restored.

The owning [raw Header](../../../iracing-irsdk/src/header.rs) and [DiskSubHeader](../../../iracing-irsdk/src/disk_sub_header.rs) define value representation and interpretation.
[DocumentWriter](../../src/writer/document.rs) owns selected serialization,
newline/framing behavior, and flushing; [OutputSink](../../src/writer/output_sink.rs)
owns file/stdout I/O. Clap owns exact options and accepted format names.
Output failures can leave a prefix written; success does not imply filesystem durability.

## Development references

[Command implementation and tests](../../src/commands/headers/mod.rs) own execution order.
The adjacent Windows fake-capability test checks live dispatch, the acquisition
error, and capability call count using stdout. Early file opening follows the
run method's order; that test does not assert file truncation. The IBT branch
opens its file directly and does not consume the injected live capability.
Check source acquisition and existing output-file behavior when changing validation.
