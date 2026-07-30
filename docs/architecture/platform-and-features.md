# Platform and Feature Boundaries

## Capability matrix

| Capability | Linux/macOS | Windows | WASM library check |
| --- | --- | --- | --- |
| `.ibt` parsing and direct replay | Yes | Yes | Must compile |
| Variable/frame/adapter types | Yes | Yes | Must compile |
| Session YAML models and parsing | Yes | Yes | Must compile |
| `IbtProvider` and `IbtConnection` | Yes | Yes | Must compile where runtime subset allows |
| Live shared-memory provider | No | Yes | No transport |
| `LiveConnection` symbol | Stub/builder only | Full implementation | Portable surface only |
| Typed broadcast commands | Yes | Yes | Must remain portable where exported |
| Win32 broadcast transport | No | Yes | No |
| Generated gRPC bindings/client | Yes | Yes | Not part of SDK WASM gate |
| Real broadcast gRPC service/server | No | Yes | No |
| Simulation HTTP probe | Yes | Yes | Not checked for WASM |
| Simulation process enumeration | No | Yes | No |

## Windows boundary

Windows-only code includes shared-memory mapping, update events, process
enumeration, and `SendNotifyMessageW` broadcast transport. Put cfg gates around
the smallest implementation layer that imports Windows APIs.

Portable public models should not disappear merely because a transport is
Windows-only. Existing examples:

- `BroadcastCommand` and related enums are portable typed data;
- generated protobuf messages and `RawBroadcastClient` are portable;
- `LiveConnection` preserves a non-Windows builder stub that fails clearly.

Windows-only binaries must retain explicit non-Windows behavior when Cargo/CI
builds all targets, and distributable binaries must be listed under matching
`package.metadata.dist.bin.*.targets`.

## SDK features

| Feature | Role |
| --- | --- |
| `derive` (default) | Re-export `iracing-sdk-derive` macros. |
| `codegen` | Add `schemars` derives/helpers and enable schema-generation binaries. |
| `schema-discovery` | Preserve and inspect unknown session YAML fields. |
| `benchmark` | Enable Criterion benchmark targets. |

Features are capabilities, not platform substitutes. In particular, live
support is selected by `cfg(windows)`, not a Cargo `live` feature.

Schema generation commonly enables both `codegen` and `schema-discovery`
because discovery overlays provide examples/unknown fields used by the tools.

## Tokio target split

Native `iracing-sdk` targets use full Tokio. `wasm32` uses a reduced,
default-features-disabled subset for synchronization, runtime, time, streams,
and cancellation support.

Keep code that only needs WASM-safe synchronization portable. Gate or restructure
APIs that require native runtime, I/O, signals, or OS threads. WASM compatibility
is not enforced by the CI workflow.

## Documentation and CI platforms

The main quality job runs on Ubuntu and Windows. SDK, derive, and simulation
documentation jobs also run on both. Linux success cannot prove the Windows
shared-memory path works, while Windows success alone can hide missing cfg
boundaries.

When adding a public API:

1. decide whether its data model is portable;
2. isolate the OS call behind an adapter/module;
3. add or preserve a clear unsupported-platform behavior;
4. check examples/binaries on both cfg paths;
5. update dist target metadata for shipped binaries;
