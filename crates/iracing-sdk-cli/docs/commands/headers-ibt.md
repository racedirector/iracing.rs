# headers ibt

Inspect the source's native header values without interpreting them as telemetry fields.

The IBT leaf is portable and requires a readable completed recording accepted by `IbtFile::open`. Source validation belongs to [IbtFile](../../../iracing-sdk/src/ibt/file.rs), not the CLI.

## Execution and side effects

Open a completed immutable recording through `IbtFile::open`, then write its header followed by its disk header. These are two consecutive values, not a combined object or array; repeated JSON writes do not form one aggregate JSON document.

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
