# Targeted Rust verification

`miri.yml` pins nightly-2026-10-01 and runs only modeled Rust surfaces: native wire
encoding/decoding, byte and specialized regions, pure physical IbtLayout geometry,
and owned-source exact reads/short-read recovery. Run the same four Miri test
commands and the separate setup step from that workflow locally. No simulator, OS-backed mmap or Win32 transport is
executed. Default Miri provenance/race checks are retained without experimental
flags; review nightly updates and rerun this subset deliberately.

**Passing Miri does not prove actual Win32 mapping extent, event lifetime,
hardware ordering, external producer behavior or coherent live snapshots.**
Current live Connection uses borrowed external references and has no isolated
owned-snapshot seam; inventing one here would redesign the transport. Live
adversarial protocol/state tests remain follow-up work for that implementation,
with native integration and proof review required independently.

Core Header, DiskSubHeader, VariableBuffer and VariableHeader sizes, alignment and
field offsets are asserted in const contexts, so ordinary builds fail on ABI
drift. Existing runtime layout and byte/serialization round-trip tests remain.
The Rust 1.88 support floor permits const assertions and offset_of.

`cargo test -p iracing-sdk-derive --test ui` checks six public diagnostic contracts:
non-struct and non-named inputs, missing/malformed field attributes, critical
optional fields, and invalid bitfield target types. A valid named-field control
must compile. These are checked by ordinary workspace tests, not a second CI
framework. To intentionally change a diagnostic, run
`TRYBUILD=overwrite cargo test -p iracing-sdk-derive --test ui`, review the small
stderr diff and rerun without overwrite. Baselines are maintained on the declared
MSRV; ordinary stable/Windows quality CI also exercises them. Do not snapshot
incidental internal type errors or weaken diagnostics merely to satisfy tests.
