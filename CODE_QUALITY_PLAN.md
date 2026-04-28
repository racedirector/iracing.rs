# Workspace Code Quality Plan (Updated)

This plan tracks quality work for `iracing.rs`, reflecting the decision to **exclude dependency/supply-chain workflow work** (`deps.yml`) for now.

## Current Status

### Completed
1. Unified baseline quality CI flow exists at `.github/workflows/quality.yml`:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets`
   - Includes cargo cache and CI diagnostics (`rustc --version`, `cargo --version`)
2. Workspace lint centralization is implemented:
   - Root `Cargo.toml` contains `[workspace.lints.rust]` and `[workspace.lints.clippy]`
   - All workspace members opt in via `[lints] workspace = true`

### Remaining
1. Add root `.pre-commit-config.yaml` with local hooks that mirror the quality CI command order:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets`
2. Pin and standardize toolchain versions:
   - Add root `rust-toolchain.toml` with a pinned channel/version and required components (`rustfmt`, `clippy`)
   - Update all workflows to use the same pinned toolchain (instead of `stable`)
3. Document contributor quality contract:
   - Add `CONTRIBUTING.md` with “Before you push” checklist matching CI/pre-commit commands
   - Include pre-commit setup (`pre-commit install`)
   - Reference `CONTRIBUTING.md` from root `README.md`
4. Mark quality workflow as a required status check in GitHub branch protection.

## Explicitly Out of Scope (Current Decision)

1. Do not add `.github/workflows/deps.yml`.
2. Do not add `deny.toml` as part of this plan phase.
3. Do not gate merges on dependency/supply-chain checks during this phase.

---

## Definition of Done (Revised)

This revised plan is complete when:
1. Local pre-commit hooks and CI run equivalent baseline checks in the same order.
2. `quality.yml` is configured as a required status check.
3. Lint and toolchain policy are centralized and consistently inherited.
4. Contributor docs reflect the exact enforced commands.
