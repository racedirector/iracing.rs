# Workspace Code Quality Improvement Plan

This plan defines concrete, enforceable quality improvements for the `iracing.rs` workspace and ensures they are consistently applied via both **pre-commit hooks** and **GitHub Actions CI**.

## 1) Add a unified baseline quality gate (all crates)

### Why
Current CI is focused on docs for `iracing-sdk` and does not provide full workspace gating for formatting, linting, and tests.

### Actions
1. Add a root `.pre-commit-config.yaml` with local hooks that run from repo root:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets`
2. Add `.github/workflows/quality.yml` that runs on pull requests and pushes to `main`.
3. Run the same commands in CI and pre-commit, in the same order, so local and CI checks stay aligned.
4. Configure Rust toolchain setup and cargo caching in the workflow to keep runtime reasonable.
5. Mark this workflow as a required status check in branch protection.

## 2) Centralize lint policy at workspace level

### Why
Lint policy can drift if each crate configures it independently.

### Actions
1. Add shared lint policy to root `Cargo.toml`:
   - `[workspace.lints.rust]`
   - `[workspace.lints.clippy]`
2. In each crate `Cargo.toml`, opt into workspace lints with:
   - `[lints]`
   - `workspace = true`
3. Keep CI/pre-commit clippy invocations workspace-wide and strict.
4. Permit per-item exceptions only with explicit, documented rationale in source comments.

## 3) Pin and standardize toolchain versions

### Why
Toolchain drift causes inconsistent results between developer machines and CI.

### Actions
1. Add root `rust-toolchain.toml` with:
   - pinned toolchain channel/version
   - required components: `rustfmt`, `clippy`
2. Use the same toolchain in all GitHub Actions workflows (including existing docs workflow).
3. Add a CI diagnostics step printing:
   - `rustc --version`
   - `cargo --version`

## 4) Add dependency and supply-chain checks

### Why
Code quality includes dependency health, license compliance, and advisory checks.

### Actions
1. Add `.github/workflows/deps.yml` with:
   - `cargo deny check advisories licenses bans sources`
   - optional `cargo audit` for cross-checking advisories
2. Add and tune root `deny.toml` with approved licenses and source/dependency constraints.
3. Make dependency checks required (or required on protected branches).
4. Keep dependency checks optional/manual in pre-commit if runtime is too slow for default local hooks.

## 5) Document contributor quality contract

### Why
A clear contract improves consistency and reduces CI churn.

### Actions
1. Add `CONTRIBUTING.md` with a “Before you push” checklist:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets`
2. Include pre-commit setup steps (`pre-commit install`).
3. Reference `CONTRIBUTING.md` from the root README.
4. Ensure documented commands exactly match hook and CI commands.

---

## Recommended rollout order

1. Baseline quality gate
2. Workspace lint policy
3. Toolchain pinning
4. Dependency/supply-chain checks
5. Contributor documentation

---

## Definition of done

This plan is considered complete when:
- Local pre-commit hooks and CI run equivalent baseline checks.
- Baseline and dependency workflows are required in branch protection.
- Lint/toolchain policy is centralized and inherited across all crates.
- Contributor docs reflect the exact enforced commands.
