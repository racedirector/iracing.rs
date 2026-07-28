# AGENTS.md

Reference for agents using `crates/test-utils`.

## Purpose

- Centralizes test-data path resolution and generated fixture guardrails; other crates should call these helpers instead of hardcoding `../../../test-data` paths.
- The guidance string `FIXTURE_INSTALL_GUIDANCE` drives all missing-fixture error messages—update it here if onboarding steps change.

## Key Helpers

- `find_git_repository_root()` + `get_test_data_dir()` walk up from CWD and fail loudly when no `.git` entry is found.
- `load_fixture_manifest()` loads generated fixture invariants from `test-data/ibt/manifest.json`.
- `require_ibt_fixtures()`, `require_named_ibt_fixture()`, and `require_smallest_ibt_fixture()` enforce generated fixture presence and return actionable errors.
- `get_ibt_test_files()` and `get_smallest_ibt_test_file()` return best-effort results without failing; use them only for explicitly optional data-driven work.

## Usage Guidelines

- When adding fixture-aware tests in other crates, import these helpers instead of rolling custom logic so missing-fixture failures stay uniform.
- Regenerate, verify, and drift-check fixtures with `python3 scripts/check_test_fixtures.py`. Use `scripts/generate_test_fixtures.py` or `--no-drift-check` only while intentionally updating generated data.
- Treat `test-data/ibt/manifest.json` as authoritative when present; unlisted `.ibt` captures are not returned by manifest-backed discovery.
- Keep Windows-only helpers such as `require_test_data_file` behind the same cfg gates as their call sites.
- Do not add heavy dependencies here; the crate should remain lightweight and build in every workspace configuration.
