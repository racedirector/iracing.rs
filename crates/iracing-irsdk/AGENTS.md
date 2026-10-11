# Wire-contract guidance

Read [crate Rustdoc](src/lib.rs) and the affected definition before editing.
Preserve adjacent ABI assertions and byte round-trip tests. This crate owns
intrinsic wire/domain contracts; source access and runtime policy belong to
consumers. Do not add unchecked enum conversions.

Run `cargo test -p iracing-irsdk --all-features`, doctests, and warnings-denied
Rustdoc. Follow root AGENTS.md for repository gates.

For proof-oriented audits compose rust-unsafe-ffi and rust-soundness-review;
start with [unsafe boundary navigation](../../docs/architecture/unsafe-boundaries.md).
Generic BitField migration remains tracked in #167; do not settle its public API
through documentation changes.
