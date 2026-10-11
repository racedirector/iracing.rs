# Benchmark evidence and procedures

Current measurement contracts live in [benchmark source and support modules](../../crates/iracing-sdk/benches/).
[Cargo](../../crates/iracing-sdk/Cargo.toml) owns target declarations;
[workflows](../../.github/workflows/) and [repository checks](../../xtask/src/workflows.rs)
own automated execution and reporting. Start with the
[operator guide](../../crates/iracing-sdk/benches/README.md) to run a current target.

Authored evidence-collection procedures remain useful here:

- [Build identity](codspeed-build-identity.md) explains how to retain provenance
  and review baseline transitions; exact configured inputs live in tooling.
- [Stability study](codspeed-stability.md) contains a repeatable procedure and a
  separately identified 2026-10-03 dataset.
- [Investigation runbook](codspeed-investigation.md) guides interpretation of a
  particular comparison without dismissing unresolved signals.

Historical records retain their original observations and limitations:

- [IBT storage measurement](ibt-reader.md): 2026-09-19 seekable-file diagnostic.
- [IBT performance snapshot](ibt-reader-performance.md): pre-refactor revision
  `8bb0f94`, with recorded compiler and environment.
- [Issue #141 mmap comparison](issue-141-mmap.md): recorded base revision and
  working-head quick-mode experiment; missing date/head identity remains explicit.
- [CodSpeed result data](data/codspeed-2026-10-03.json): retained measurement
  values and run provenance, interpreted by the stability report.

Old implementation names and reproduction commands describe their recorded
experiments. They do not define current architecture or target vocabulary.
Missing provenance is a limit on comparability, not a field to reconstruct by
guessing. Preserve retained results; collect a new identified run for new claims.
