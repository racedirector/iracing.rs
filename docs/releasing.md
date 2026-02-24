# Releasing `iracing.rs` with `dist`

This repository uses standalone `dist` (not `cargo dist`) to publish binaries to GitHub Releases and formulas to Homebrew.

## What Gets Released

- Package: `iracing-sdk`
- Package: `iracing-sdk-codegen`
- Targets:
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
  - `aarch64-apple-darwin`
- Installer channel:
  - Homebrew tap: `racedirector/homebrew-tap`

## Required GitHub Secrets

- `HOMEBREW_TAP_TOKEN`
  - Personal access token with write access to `racedirector/homebrew-tap`.
  - Used by `publish-homebrew-formula` job in [release.yml](/Users/justinmakaila/Developer/iracing.rs/.github/workflows/release.yml).

## Local Validation Before Tagging

Run from repository root:

```bash
cargo check -p iracing-sdk --bins
cargo check -p iracing-sdk-codegen --bins
```

Validate release planning (same targets/installers used by CI):

```bash
dist plan \
  --tag v0.1.0 \
  --target x86_64-unknown-linux-gnu \
  --target x86_64-pc-windows-msvc \
  --target aarch64-apple-darwin \
  --installer homebrew
```

Optional local CI config integrity check:

```bash
dist generate --mode ci --check --allow-dirty
```

## Release Flow

### 1. Bump versions

Keep `iracing-sdk` and `iracing-sdk-codegen` in lockstep.

### 2. Prerelease (recommended first)

Push a prerelease tag:

```bash
git tag vX.Y.Z-rc.1
git push origin vX.Y.Z-rc.1
```

Then verify:

- GitHub Release artifacts are created.
- Homebrew formula PR/commit reaches `racedirector/homebrew-tap`.
- Install/test from artifacts and Homebrew.

### 3. Stable release

Push stable tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The release workflow is triggered by `v*` tags and can also be run manually with `workflow_dispatch` and an explicit tag input.

## Manual Retry / Backfill

Use `workflow_dispatch` in GitHub Actions and pass the target tag (for example `v0.1.0`).

## Rollback Strategy

- If artifacts are wrong but tag already exists:
  - Delete/replace GitHub Release assets.
  - Re-run workflow with `workflow_dispatch` for the same tag after fixes.
- If tag itself is wrong:
  - Delete the Git tag locally/remotely and create a corrected tag.
- If Homebrew formula push is wrong:
  - Revert the tap commit in `racedirector/homebrew-tap`.
  - Re-run workflow for the corrected release.

## First-Release Checklist

- `HOMEBREW_TAP_TOKEN` is configured in repository secrets.
- `racedirector/homebrew-tap` exists and is writable by token owner.
- `dist plan` succeeds locally.
- `release.yml` manual run succeeds for a prerelease tag.
- End-to-end install test passes from both GitHub artifacts and Homebrew.
