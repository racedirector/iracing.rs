# iracing-irsdk

Dependency-light native iRacing SDK wire and domain types.

Use this crate for raw SDK headers, metadata, constants, enums, flags, and
broadcast values. For recording access, live transport, session parsing, or
telemetry extraction, start with `iracing-sdk`.

The [crate Rustdoc](src/lib.rs) describes representation and validation boundaries.
Follow [Header](src/header.rs), [IbtHeader](src/ibt_header.rs),
[VariableHeader](src/variable_header.rs), [VariableType](src/variable_type.rs),
[flags](src/flags.rs), and [raw broadcast values](src/broadcast.rs) for contracts
and their adjacent tests. ABI assertions remain with the owning definitions.

```rust
use iracing_irsdk::VariableType;

let storage = VariableType::try_from(3).expect("known SDK storage type");
assert_eq!(storage, VariableType::BitField);
```

`iracing-sdk` also exposes these types through `iracing_sdk::irsdk`.
Cargo.toml owns optional feature configuration.
