# Wire-contract guidance

Run `cargo test -p iracing-irsdk --all-features`, doctests and docs-as-warnings
when changing public wire contracts. This crate owns dependency-light native SDK
representation/decoding, not source access, IBT navigation or runtime orchestration.

Preserve exact ABI size/alignment/offset invariants and byte round-trip tests.
Distinguish arbitrary-bit-pattern validity from constrained enum decoding,
semantic field validation and cross-field/source layout. FromBytes is not a
certificate that counts/offsets are usable. Do not add unchecked enum conversion.

Compose `rust-unsafe-ffi` with `rust-soundness-review` for proof-oriented audits;
trace all constructors/deserialization/mutations to consumers. Consult
`docs/architecture/unsafe-boundaries.md` from the workspace root. Miri proves only
modeled Rust executions, not real Win32/external producer behavior. Performance
claims require relevant caller/benchmark inspection and measurement.
