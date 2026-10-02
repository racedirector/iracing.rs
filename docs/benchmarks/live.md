# Local live telemetry measurements

These diagnostics require Windows and an active iRacing session. They observe
real shared-memory acquisition and watch-backed subscriber delivery. Results
depend on simulator pacing, Windows scheduling, session activity, and other
machine load; they are not Criterion or CodSpeed timings. No hosted CI job runs
an active simulator.

## Measurement boundaries

`live_acquisition/owned_frame` waits for the next event outside the timer and
times `WindowsConnection::get_new_data()`, the immediate owned byte copy, and
the same tick/session-version reads used by `LiveProvider` on pre-#111 `main`.
Only accepted operations contribute to p50/p95/p99. No-frame attempts, event
signals/timeouts, skipped ticks, copied bytes, and effective accepted rate are
reported separately. The operation is not producer-to-consumer latency or a
guaranteed frame-ready latency. Its experiment version must change if its timed
boundary changes. The case ID remains stable through #111's source API change.

`live_consumer/dynamic_1` preserves the existing native-rate `DynamicFrame`
subscription. `live_consumer/dynamic_4` creates four simultaneous subscriptions
on the same `LiveConnection`. Connection and subscription setup precede the
observation window. At least 120 source ticks are discarded, followed by at
least 600 source tick opportunities. Each subscriber reports received frames,
inter-arrival percentiles, effective rate, and skipped/coalesced ticks. Delivery
is latest-wins: a replaced intermediate frame is not an acquisition failure.
Inter-arrival time is cadence, not one-way end-to-end latency.

Both targets write format-versioned JSON with exact Git SHA, dirty state,
non-sensitive machine profile, Windows/CPU/Rust/build/power identity, source
rate, frame size, and a stable schema fingerprint. A timeout, disconnection,
geometry or session change, or insufficient samples makes a case incomplete.
Incomplete runs are retained for diagnosis but cannot qualify as baselines.
Neither target stores telemetry payloads or session YAML.

## Capture pre-#111 and candidate revisions

Build current `main` and candidate in separate worktrees using the same Rust
toolchain, target, and bench profile. Build **before** timed runs:

```powershell
git worktree add ../iracing-pre111 main
git worktree add ../iracing-candidate <candidate-branch>
cd ../iracing-pre111
cargo bench -p iracing-sdk --features benchmark --bench live-acquisition-diagnostic --bench live-telemetry-diagnostic --no-run
cd ../iracing-candidate
cargo bench -p iracing-sdk --features benchmark --bench live-acquisition-diagnostic --bench live-telemetry-diagnostic --no-run
```

Keep the same installed simulator, active session, source tick rate, power
profile, and minimal background load. Run each case at least three times on
each revision. Alternate base/head executable runs where practical to expose
thermal or session drift. Retain all run IDs; compare medians of corresponding
run-level statistics. Document any excluded outlier and its reason. Even on
the same PC, simulator-backed results remain environment-sensitive.

From each worktree, run both diagnostics (the `--` passes options to the
target). A machine label should describe hardware without naming a person or
machine account:

```powershell
cargo bench -p iracing-sdk --features benchmark --bench live-acquisition-diagnostic -- --profile win11-desktop-a
cargo bench -p iracing-sdk --features benchmark --bench live-telemetry-diagnostic -- --profile win11-desktop-a
```

Default full summaries go to `target/live-benchmarks/runs/<run-id>.json`.
Use `--output <path>` for an explicit location. `--warmup-frames 120`,
`--target-frames 600` (minimum 600), and `--timeout-seconds 60` can be tuned
for a steady session. The acquisition case and two consumer cases are separate
run files.

The local record command copies a run out of `target` into ignored
`.live-benchmarks/runs/`, so `cargo clean` and worktree rebuilds do not remove
it. It never overwrites an existing run or label. Save the qualifying clean
pre-#111 acquisition and one-consumer run IDs under explicit labels:

For a store shared by both worktrees, set `IRACING_LIVE_BENCH_STORE` to a
durable directory outside either worktree before invoking the script. The
default is `.live-benchmarks` in the current worktree.

```powershell
python scripts/live_benchmarks.py record target/live-benchmarks/runs/<acquisition-id>.json --label pre-111-acquisition
python scripts/live_benchmarks.py record target/live-benchmarks/runs/<consumer-id>.json --label pre-111-consumer
```

Keep that `.live-benchmarks` store on a durable local drive when removing a
worktree; copying it to the candidate worktree is sufficient. Compare explicit
files or a recorded baseline label, and save both the Markdown and JSON delta:

```powershell
python scripts/live_benchmarks.py compare --base .live-benchmarks/runs/<base-id>.json --head .live-benchmarks/runs/<head-id>.json --json-output delta.json --markdown-output delta.md
python scripts/live_benchmarks.py compare --baseline pre-111-acquisition --head .live-benchmarks/runs/<head-id>.json
```

Comparison pairs only equal case IDs, experiment versions, machine hardware,
target/build/power profile, source rate, frame geometry/fingerprint, workload
version, and case parameters. Missing/new cases and changed experiments are
unpaired. Other mismatches are incomparable. Changed non-identifying simulator
conditions, if recorded, make a paired delta diagnostic rather than confirmed
regression evidence. Never compare these wall-time observations numerically
with CodSpeed simulation results.

For a maintainer-reviewed PR, explicitly promote each complete, clean,
sanitized summary. The command refuses to overwrite an existing file:

```powershell
python scripts/live_benchmarks.py promote .live-benchmarks/runs/<run-id>.json
```

This writes a small JSON file under
`docs/benchmarks/results/live/<profile>/<run-id>.json`. Review it before
committing; include a short Markdown comparison in the PR or issue. Raw
per-frame samples stay local. A qualified pre-#111 acquisition and one-consumer
result must be promoted and linked from issue #160 before #111 changes the
Windows source API.
