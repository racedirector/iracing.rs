# Live telemetry shared-memory format

This document specifies how to locate, validate, and snapshot iRacing live telemetry from its Windows memory-mapped file. **MUST** and **MUST NOT** are requirements; **SHOULD** permits a documented exception. Offsets are absolute from the beginning of the mapped view and ranges are half-open. The source protocol is the local iRacing SDK 1.20 [`irsdk_defines.h`](../../irsdk_1_20/irsdk_defines.h), [`irsdk_utils.cpp`](../../irsdk_1_20/irsdk_utils.cpp), and [memory server](../../irsdk_1_20/irsdk_server/irsdk_memserver.cpp). [Observed repository artifacts](live-telemetry-observations.md) and [implementation notes](live-telemetry-parser-notes.md) are separate.

## Windows objects and mapping

The producer publishes a page-file-backed mapping named `Local\IRSDKMemMapFileName` and signals `Local\IRSDKDataValidEvent` after publishing telemetry. A reader MUST open the mapping for read access and MUST treat it as volatile shared state. It MUST know or determine the mapped view length before constructing slices from advertised offsets. Opening either Windows object does not establish that a telemetry session is connected.

Multi-byte values MUST be interpreted as little-endian. The mapped view begins with one 112-byte `irsdk_header` at offset 0. Unlike an IBT file, live shared memory has no `irsdk_diskSubHeader` and no linear frame stream. The header locates one mutable session-information region, one variable-header array, and three or four rotating telemetry buffers.

Reads from the mapping MUST use primitives whose semantics preserve accesses to concurrently changing shared memory; an implementation MUST NOT let the compiler cache, elide, or reorder the protocol reads around the required barriers. Header values used for range construction SHOULD be copied into reader-owned storage and validated as one layout observation. The reader MUST confirm connected status after observing the layout and MUST abandon that observation if disconnection or a layout change is detected before use.

| Region | Absolute range |
| --- | --- |
| Main header | `[0,112)` |
| Session-information storage | `[sessionInfoOffset, sessionInfoOffset + sessionInfoLen)` |
| Variable headers | `[varHeaderOffset, varHeaderOffset + numVars × 144)` |
| Active telemetry buffer `i` | `[varBuf[i].bufOffset, varBuf[i].bufOffset + bufLen)` |

Every present region MUST fit entirely within the mapped view. Its offset, length, multiplication, and end calculation MUST be checked before pointer arithmetic or slicing. The header, session region, variable-header array, and every active telemetry buffer MUST NOT overlap. Active telemetry buffers MUST NOT overlap one another. The layout MAY contain reserved space or gaps and MUST be followed through offsets rather than reconstructed by adjacency.

## Main header: `irsdk_header` (112 bytes)

Except where specified, fields are signed 32-bit integers. A reader MUST reject an unsupported `ver`; this specification describes version 2. A connected layout MUST have a positive `tickRate`, positive `bufLen`, nonnegative region geometry, and `numBuf` in `3..=4`. `curBuf` is an unsigned byte and MUST be less than `numBuf`. The connected status is bit 0 (`status & 1 != 0`). A mapped but disconnected header MUST NOT be used to publish telemetry.

| Offset | Field | Meaning |
| ---: | --- | --- |
| 0 | `ver` | SDK header version. |
| 4 | `status` | Status bitfield; bit 0 means connected. |
| 8 | `tickRate` | Nominal producer samples per second. |
| 12 | `sessionInfoUpdate` | Incremented after a changed session string is published. |
| 16, 20 | `sessionInfoLen`, `sessionInfoOffset` | Storage length and absolute offset of session information. |
| 24, 28 | `numVars`, `varHeaderOffset` | Count and absolute offset of variable headers. |
| 32, 36 | `numBuf`, `bufLen` | Active rotating-buffer count and bytes in each frame. |
| 40 | `curBufTickCount` | Cached tick of the most recently published buffer. |
| 44 | `curBuf` | Index of the most recently published buffer; bytes 45–47 are padding. |
| 48–111 | `varBuf[4]` | Four 16-byte buffer descriptors. Only the first `numBuf` are active. |

The producer publishes the layout before setting the connected bit. It clears that bit on shutdown. A reader MUST validate the complete connected header before using its offsets. Schema and buffer geometry MAY be cached only for the lifetime of that connected layout; a disconnect, remap, unsupported header, or changed layout field invalidates the cache.

## Buffer descriptors: `irsdk_varBuf` (16 bytes each)

| Relative offset | Field | Type | Publication role |
| ---: | --- | --- | --- |
| 0 | `tickCount` | `i32` | Written after the complete frame. |
| 4 | `bufOffset` | `i32` | Absolute offset of this buffer. |
| 8 | `tickCountBegin` | `i32` | Written before writing the frame. |
| 12 | padding | 4 bytes | No semantic value. |

The producer rotates through active buffers. For tick `T`, it writes `tickCountBegin = T`, executes a write barrier, writes the frame, executes a write barrier, writes `tickCount = T`, updates `curBufTickCount` and `curBuf`, executes a write barrier, and signals the event. Tick counters are signed and MAY reset or wrap; a reader MUST NOT assume they are permanently increasing across a disconnect or reset.

## Acquiring a consistent telemetry frame

The frame bytes are volatile and MUST be copied into reader-owned storage before parsing or publishing them. A conforming snapshot operation is:

1. Read and validate a connected header view.
2. Select `curBuf` as the most recently published buffer. If it is outside the active range, the header is not a valid connected snapshot and MUST NOT be dereferenced.
3. Read that descriptor's `tickCount` as `T`.
4. Execute a compiler/CPU read barrier sufficient for the platform and copy exactly `bufLen` bytes from its validated `bufOffset` range into owned storage.
5. Execute another read barrier and read the same descriptor's `tickCountBegin`.
6. Accept the copy only if `tickCountBegin == T`. Otherwise discard it and retry from a fresh header/buffer selection.

Equality establishes that the producer did not begin overwriting that rotating buffer during the copy. Comparing `tickCount` only with another read of `tickCount` is insufficient under the version-2 publication protocol. A borrowed slice into the mapping MUST NOT escape the consistency check because it can change immediately afterward.

A consumer tracking new frames SHOULD retain the tick of its last accepted snapshot. An equal tick is not new. A lower or otherwise discontinuous tick MUST be treated as a reset/wrap boundary rather than silently ordered before the old session. The protocol is latest-state delivery: if the producer advances through multiple ticks between reads, unavailable intermediate frames cannot be recovered from the rotating buffers.

## Update event

The data-valid event is a wake-up hint, not a frame payload or queue. A reader SHOULD check for an already available new tick before waiting, wait with a bounded or cancellable policy, then re-read shared memory after either a signal or timeout. A signal MAY coincide with initialization or another shared-state change and MUST NOT cause publication without a fresh connected-header and stable-frame check. Multiple producer updates MAY coalesce into one observable wake-up.

## Variable headers: `irsdk_varHeader` (144 bytes each)

The variable-header wire format and storage types are identical to [the IBT variable-header format](ibt-spec.md#variable-headers-irsdk_varheader-144-bytes-each). There are `numVars` consecutive headers. Each variable offset is relative to the start of every telemetry buffer. A reader MUST reject unknown or sentinel types, negative offsets, nonpositive counts, empty or duplicate names, arithmetic overflow, or a variable extent beyond `bufLen`.

The SDK contract fixes the variable list after the connected session starts. A reader SHOULD copy the advertised variable-header bytes and build a validated immutable schema once per connected layout. It MUST NOT decode owned frame bytes using a schema captured from a different layout.

## Session information

The session region is mutable, NUL-terminated YAML stored within a fixed advertised capacity. `sessionInfoLen` bounds the storage region; it is not necessarily the current YAML payload length. The first NUL ends the payload. `sessionInfoUpdate` is incremented only after the producer writes changed session bytes and executes a write barrier.

A reader that associates bytes with a session revision MUST read revision `S1`, copy the entire validated region into owned storage, execute the required read barrier, and read revision `S2`. It MUST accept the snapshot for that revision only when `S1 == S2`; otherwise it MUST discard and retry. Because the mapping holds only the current string, revisions overwritten before they are copied cannot be reconstructed. Telemetry and session information have separate publication mechanisms; a reader MUST NOT claim they form one atomic snapshot.

Session decoding follows [the IBT session-information encoding rules](ibt-spec.md#session-information): recognize `WeekendInfo.Encoding` values `UTF8`, `UTF-8`, and `ISO_8859_1`; otherwise try UTF-8 and fall back to ISO-8859-1. Binary range validation and owned copying MUST precede decoding and YAML parsing.

## Published result

A published telemetry sample MUST contain the owned, consistency-checked frame bytes and the `tickCount` used in that check. Its schema MUST have been validated against the same connected layout and `bufLen`. A separately copied session snapshot MAY be associated with the observed `sessionInfoUpdate`, but the association is observational rather than atomic. Typed value decoding occurs from owned frame bytes using the validated schema and little-endian SDK storage types.
