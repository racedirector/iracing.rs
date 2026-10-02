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
