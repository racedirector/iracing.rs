# iracing-irsdk

Dependency-light Rust representations of the native iRacing SDK wire contract.

Use this crate when you need SDK constants, fixed-layout headers, variable
metadata, enums, flags, broadcast command values, or intrinsic wire decoding
without the file parsing, shared-memory transport, session parsing, schema, or
streaming layers provided by `iracing-sdk`.

```rust
use iracing_irsdk::{Header, VariableHeader, VariableType, WireType};

assert_eq!(Header::WIRE_SIZE, 112);
assert_eq!(VariableHeader::WIRE_SIZE, 144);
assert_eq!(VariableType::Double.byte_size(), Some(8));
```

`iracing-sdk` depends on this crate and re-exports it through its existing
`iracing_sdk::irsdk` module. Applications already using that namespace do not
need to change their imports.

## Boundary

Types belong here when they make sense given only the native SDK definitions
and documented byte layout. Source navigation and runtime behavior do not:

- `.ibt` file parsing and seeking stay in `iracing-sdk`;
- Windows shared-memory access stays in `iracing-sdk`;
- telemetry schemas, frames, providers, and connections stay in `iracing-sdk`;
- session YAML decoding and sanitization stay in `iracing-sdk`.

The optional `codegen` feature adds `schemars` implementations used by the
higher-level crate's schema-generation tools.
