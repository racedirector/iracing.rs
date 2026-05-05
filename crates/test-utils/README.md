# test-utils

Shared testing helpers for the `iracing.rs` workspace.

This crate exists to keep integration tests consistent across crates by centralizing:

- workspace root discovery (find the git checkout from any CWD)
- `test-data/` path resolution
- generated `.ibt` fixture discovery and actionable regeneration messaging

## Fixture layout

The `.ibt`-specific helpers (`get_ibt_test_files`, `require_ibt_fixtures`, …) prefer the generated fixture manifest:

- `test-data/ibt/manifest.json`

Manifest-listed fixtures are authoritative when the manifest exists. Regenerate them from the repository root with:

```bash
python3 scripts/check_test_fixtures.py
```

## Key APIs

All public symbols are exported from `src/lib.rs`:

- `FIXTURE_INSTALL_GUIDANCE`: shared guidance string appended to fixture-related errors.
- `FixtureError`: lightweight error type used by the `require_*` helpers.
- `load_fixture_manifest()`: load generated fixture invariants for parser/reader tests.
- `find_git_repository_root()`: locate the git checkout root by walking up for `.git/`.
- `get_test_data_dir()`: resolve `test-data/` relative to the git root.
- `.ibt` discovery:
  - `get_ibt_test_files() -> Vec<PathBuf>`
  - `get_smallest_ibt_test_file() -> Option<PathBuf>`
  - `require_ibt_fixtures() -> Result<Vec<PathBuf>, FixtureError>`
  - `require_named_ibt_fixture(name) -> Result<PathBuf, FixtureError>`
  - `require_smallest_ibt_fixture() -> Result<PathBuf, FixtureError>`

## Example usage (in another crate’s tests)

```rust,no_run
use test_utils::require_smallest_ibt_fixture;

#[test]
fn can_open_a_fixture() {
    let path = require_smallest_ibt_fixture().expect("fixtures must be present");
    println!("fixture={}", path.display());
}
```

## Design constraints

- Keep this crate lightweight: it should compile under any workspace configuration and shouldn’t pull in heavy dependencies.
- Prefer using these helpers over hardcoded `../../../test-data/...` paths so missing-fixture failures remain actionable and uniform.
