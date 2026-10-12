# SDK benchmark operator guide

Use each benchmark's source-level documentation for its setup, timed boundary,
throughput unit, and interpretation limits. Shared helpers under [support](support/)
own workload construction and validation. [Cargo target declarations](../Cargo.toml)
own executable names, feature requirements, and source paths; inspect them when
choosing a target rather than relying on a second inventory here.

From the repository root:

```text
cargo bench -p iracing-sdk --features benchmark --no-run
cargo bench -p iracing-sdk --features benchmark --bench <target>
```

Build before comparing runs. Retain the exact revision, toolchain/build inputs,
fixture/workload identity, runner environment, and raw report. Compare equivalent
experiments on the same machine; repeat surprising results. A changed timed
boundary or workload requires explicit comparison review. Simulation and
wall-time values describe different measurement domains.

The [benchmark workflows](../../../.github/workflows/) own automated execution
and reporting. [Repository checks](../../../xtask/src/workflows.rs) enforce
coverage and build-identity policy. Diagnostics include instrumented or live
observations; they do not automatically establish Criterion regressions. Real
simulator diagnostics require Windows and an active session, and remain manual.

For CodSpeed review, follow the [build identity guidance](../../../docs/benchmarks/codspeed-build-identity.md),
[same-revision stability procedure](../../../docs/benchmarks/codspeed-stability.md),
and [investigation runbook](../../../docs/benchmarks/codspeed-investigation.md).
These procedures guide evidence collection; workflows and tooling own exact
configured versions and checks.

[Retained benchmark records](../../../docs/benchmarks/README.md) distinguish
historical measurements from current source methodology. Do not treat old point
estimates, commands, architecture names, or recorded variance as current contracts.
