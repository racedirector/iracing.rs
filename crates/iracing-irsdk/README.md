# iracing-irsdk

Dependency-light Rust representations of the native iRacing SDK wire contract.

Use this crate when you need SDK constants, fixed-layout headers, variable
metadata, enums, flags, broadcast command values, or intrinsic wire decoding
without the file parsing, shared-memory transport, session parsing, schema, or
streaming layers provided by `iracing-sdk`.

```rust
use std::mem::size_of;

use iracing_irsdk::{Header, VariableHeader, VariableType};

assert_eq!(size_of::<Header>(), 112);
assert_eq!(size_of::<VariableHeader>(), 144);
assert_eq!(VariableType::Double.byte_size(), 8);
```

`iracing-sdk` depends on this crate and re-exports it through its existing
`iracing_sdk::irsdk` module. Applications already using that namespace do not
need to change their imports.

`Header::try_from_bytes`, `DiskSubHeader::try_from_bytes`, and the other wire
types' `try_from_bytes` methods copy exact-size values from any byte alignment.
`Header::try_from_reader` and `DiskSubHeader::try_from_reader` read one owned
value from a stream and return `Error::Io` on read failure, including truncated
input. These operations use derive-checked `zerocopy` traits. Most do not
validate SDK field values; `VariableHeader::try_from_bytes` validates its
`VariableType` discriminant. The SDK format is little-endian; these
native-layout copies require a little-endian target and do not swap bytes.

## Boundary

Types belong here when they make sense given only the native SDK definitions
and documented byte layout. Source navigation and runtime behavior do not:

- `.ibt` file parsing and seeking stay in `iracing-sdk`;
- Windows shared-memory access stays in `iracing-sdk`;
- telemetry schemas, frames, providers, and connections stay in `iracing-sdk`;
- session YAML decoding and sanitization stay in `iracing-sdk`.

The optional `codegen` feature adds `schemars` implementations used by the
higher-level crate's schema-generation tools.
