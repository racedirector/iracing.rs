# AGENTS.md

Reference for agents using `crates/test-utils`.

## Purpose
- Centralizes test-data path resolution and generated fixture guardrails; other crates should call these helpers instead of hardcoding `../../../test-data` paths.
- The guidance string `FIXTURE_INSTALL_GUIDANCE` drives all missing-fixture error messages—update it here if onboarding steps change.

## Key Helpers
- `find_git_repository_root()` + `get_test_data_dir()` walk up from CWD; they fail loudly when the repo is not checked out with `.git`.
- `load_fixture_manifest()` loads generated fixture invariants from `test-data/ibt/manifest.json`.
- `require_ibt_fixtures()`, `require_named_ibt_fixture()`, `require_smallest_ibt_fixture()` enforce generated fixture presence and return actionable errors.
- `get_ibt_test_files()`/`get_smallest_ibt_test_file()` return best-effort lists without failing; use them for optional data-driven tests.

## Usage Guidelines
- When adding new fixture-aware tests in other crates, import these helpers instead of rolling custom logic so missing-fixture failures stay uniform.
- Regenerate and verify fixtures with `python3 scripts/check_test_fixtures.py`.
- Keep Windows-only helpers (e.g. `require_test_data_file`) behind the same cfg gates as their call sites to preserve cross-platform builds.
- Do not add heavy dependencies here; the crate should remain lightweight and build in every workspace configuration.
