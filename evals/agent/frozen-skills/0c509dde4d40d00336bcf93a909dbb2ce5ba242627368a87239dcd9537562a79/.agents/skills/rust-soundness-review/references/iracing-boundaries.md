# Repository boundary routing

Start with `docs/architecture/unsafe-boundaries.md`, then inspect current source:

- `crates/iracing-sdk/src/windows/connection.rs`: handles, mapped pointers,
  borrowed external bytes, event waits, manual thread traits and frame selection.
- `crates/iracing-sdk/src/ibt/reader/`: read-only mapping versus owned bytes;
  source length, exact reads and delegated physical geometry.
- `crates/iracing-sdk/src/types/regions/` and `types/ibt/layout.rs`: geometry
  guarantees and their consumers, not blanket "validated bytes" claims.
- `crates/iracing-irsdk/src/`: byte-valid wire contracts versus semantic values.

These paths are routing clues. If code moves, follow actual exports/callers.
Keep disk immutability and live external mutation separate. Inspect benchmark
manifests, timed boundaries and workflow triggers before claiming runtime costs.
