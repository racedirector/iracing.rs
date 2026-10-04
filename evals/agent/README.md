# Agent guidance regression corpus

Eight small frozen source/contracts cases model recurring repository review risks.
Inputs live under `fixtures`; evaluator-only proposition atoms and hard failures
live under `oracles`. Case identifiers are neutral. Give agents only one prepared
treatment directory and the frozen skill condition, never oracles, sibling case
labels, fixing diffs or evaluator commentary. At least two controls penalize
invented findings. Source excerpts are review inputs, not production implementations.

Run `cargo xtask agent check` for corpus integrity, then `freeze` before
collecting any responses. Frozen packages are content-addressed and never edited
in place after failures. The first revision is committed under `frozen-skills`.
Compare it with `baseline` (no specialized skills). Later freeze a new revision
and compare it with the previous hash, retaining both. Preparation does not invoke
an agent or access credentials. No stochastic model execution gates CI.

The CLI uses the workspace clap dependency. Python 3.10+ runs the internal
evaluation backend; `PYTHON` can override the default `python3` executable
(for example, `python` on Windows). Paths are relative to the invoking directory.
Do not invoke `run.py` directly; it accepts an internal JSON request on stdin.

Example manual workflow:

```
cargo xtask agent freeze
cargo xtask agent prepare --run evals/agent/runs/baseline --model MODEL --tools TOOLS --condition baseline
cargo xtask agent prepare --run evals/agent/runs/treatment --model MODEL --tools TOOLS --condition HASH
```

Invoke the same model/tool setup independently for each case/condition, giving it
only the prepared case and, for treatment, `skills/`. Save each response as
`responses/case-NN.md`. Do not expose evaluator directories to the evaluated agent.
Adjudicate `judgments.json` against the oracles: every atom needs boolean `pass`
and concrete `evidence`; every hard failure needs boolean `triggered` and evidence.
Then run `score --run PATH` for each and `compare --left PATH --right PATH`.
Scores count propositions, not keywords. Any triggered hard failure fails the case.
The driver refuses missing responses or incomplete judgments. Model, tool, source
revision, skill hash, corpus and oracle digests, condition and timestamp are recorded
in each run. Scoring verifies the copied skills and treatments plus the current
corpus and oracles against those hashes. Runs prepared without digests must be
prepared again. Comparisons require matching model and tool values. Runs are
ignored by Git; preserve/report selected runs deliberately, without secrets.

A passing integrity check is not evidence of agent performance. No model evaluation
has been run merely by committing this corpus. Human adjudication is intentional.
