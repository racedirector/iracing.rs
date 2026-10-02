# Local verification

Run `cargo xtask check-repo` for structural repository invariants. The same
implementation runs in quality CI on Ubuntu and Windows. It verifies benchmark
filter prefixes and coverage of production input trees; changing the policy
requires inspecting current benchmark callers. It does not measure performance.
Register sibling checks in `xtask::check_repo`; keep their implementations scoped.

Run `cargo xtask pre-push` for the sequential broad gate: repository checks,
fixture regeneration/verification/drift, formatting, all-feature Clippy, workspace
tests, and benchmark compilation. Cargo builds run sequentially to avoid shared
build-directory contention. Fixtures require Git LFS hydration. Docs/public API
changes additionally require the matching commands in `AGENTS.md`/docs CI.

Hooks are optional: run the command manually or configure
`git config core.hooksPath githooks`. Neither substitutes for both native CI OSes.

## Dependency and workflow security

Dependabot remains the Cargo/Actions update mechanism. Dependency review runs on
all PRs and blocks newly introduced advisories at moderate severity or higher.
`dependency-review.yml` also runs pinned Zizmor 1.30.1 with offline analysis,
medium-or-higher severity and high confidence. Locally install that version with
`python -m pip install zizmor==1.30.1` and run
`zizmor --offline --min-severity medium --min-confidence high .github/workflows`.
No low-severity style findings are merge gates. Existing external Actions use
verified full commit pins with version comments, maintained by Actions Dependabot.
Workflow defaults are read-only; CodSpeed retains its required OIDC write scope
and release hosting alone retains contents write. Checkouts do not persist Git
credentials; publishing uses the explicit job token rather than checkout state.
