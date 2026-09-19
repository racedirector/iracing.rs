# IBT file format

This document specifies the binary layout and parsing rules for an iRacing binary telemetry (`.ibt`) file. **MUST** and **MUST NOT** are requirements; **SHOULD** permits a documented exception. Offsets are absolute from the start of the file and ranges are half-open. The source format is defined by the local iRacing SDK 1.20 [`irsdk_defines.h`](../../irsdk_1_20/irsdk_defines.h) and [disk writer](../../irsdk_1_20/irsdk_diskclient.cpp). [Capture observations](ibt-observations.md) and [parser implementation notes](ibt-parser-notes.md) are separate from this specification.

## Layout

Multi-byte numbers MUST be read as little-endian. A complete IBT begins with a 112-byte `irsdk_header` at `[0,112)` and a 32-byte `irsdk_diskSubHeader` at `[112,144)`. A parser MUST reject a file shorter than 144 bytes. The 144-byte combined preamble is not the size of the main header.

| Region | Absolute range |
| --- | --- |
| Main header | `[0,112)` |
| Disk sub-header | `[112,144)` |
| Variable headers | `[varHeaderOffset, varHeaderOffset + numVars × 144)` |
| Session information | `[sessionInfoOffset, sessionInfoOffset + sessionInfoLen)` |
| Telemetry frames | `[frameStart, fileLength)` |

The variable and session regions MUST be located independently using their advertised offsets. They MAY appear in either order. Present regions MUST NOT overlap the preamble or each other and MUST fit within the file. Conversions and arithmetic for offsets, counts, lengths, and range ends MUST be checked before indexing. A zero-length region is absent; its offset does not determine `frameStart`. `frameStart` is the greatest end of the present metadata regions, or 144 if neither is present. This parsing contract does not require the variable and session regions to be adjacent.

The SDK header defines region offsets and the disk writer emits the regions contiguously, then records the first frame in `varBuf[0].bufOffset`; it does not define a separate telemetry-region offset/length pair. Using the latest metadata end as `frameStart` is therefore the repository's compatibility rule for offset-based files, corroborated by the writer and captures rather than by an explicit SDK formula. Readers MAY compare a nonzero `varBuf[0].bufOffset` with the derived start, but it is live-style metadata and is not authoritative for bounds.

## Main header: `irsdk_header` (112 bytes)

Except where specified, fields are signed 32-bit integers. A parser MUST reject an unsupported `ver`; this specification describes SDK version 2. Counts, lengths, and offsets used for file geometry MUST be nonnegative. `bufLen` and `tickRate` MUST be positive for a conforming telemetry recording.

| Offset | Field | Meaning |
| ---: | --- | --- |
| 0 | `ver` | SDK header version. |
| 4 | `status` | 32-bit status bitfield. |
| 8 | `tickRate` | Nominal samples per second. |
| 12 | `sessionInfoUpdate` | Session-information revision. |
| 16, 20 | `sessionInfoLen`, `sessionInfoOffset` | Byte length and absolute offset of YAML region. |
| 24, 28 | `numVars`, `varHeaderOffset` | Count and absolute offset of variable-header array. |
| 32, 36 | `numBuf`, `bufLen` | Buffer count and bytes per telemetry frame. |
| 40 | `curBufTickCount` | Cached live-style tick counter. |
| 44 | `curBuf` | One-byte current-buffer index; bytes 45–47 are padding. |
| 48–111 | `varBuf[4]` | Four 16-byte buffer descriptors. |

Each `varBuf` descriptor holds `tickCount` at relative offset 0, `bufOffset` at 4, `tickCountBegin` at 8, and four padding bytes at 12. These descriptors are live-style metadata. A parser MUST NOT infer the stored frame count from their tick counters. The disk writer sets `varBuf[0].bufOffset` to the first frame; a parser MAY check it for consistency, but MUST derive frame geometry from the bounded metadata regions, `bufLen`, and file length.

## Disk sub-header: `irsdk_diskSubHeader` (32 bytes)

| Relative offset | Field | Type | Meaning |
| ---: | --- | --- | --- |
| 0 | `sessionStartDate` | signed 64-bit integer | Unix timestamp. |
| 8 | `sessionStartTime` | IEEE-754 `f64` | Session time in seconds at recording start. |
| 16 | `sessionEndTime` | IEEE-754 `f64` | Session time in seconds at recording end. |
| 24 | `sessionLapCount` | `i32` | Writer's lap count. |
| 28 | `sessionRecordCount` | `i32` | Writer's record count. |

The sub-header MUST be read at byte 112, not located relative to `varHeaderOffset`. The disk writer increments `sessionRecordCount` as it writes records, so for a finalized file it SHOULD agree with the complete-frame count derived from file length. It is advisory to readers: a mismatch is a consistency signal and MUST NOT change frame boundaries or authorize reading beyond the EOF-derived complete-frame region.

## Variable headers: `irsdk_varHeader` (144 bytes each)

There are exactly `numVars` consecutive records. Each variable's `offset` is relative to the beginning of **each frame**, not the file.

| Relative offset | Field | Type / extent |
| ---: | --- | --- |
| 0 | `type` | `i32` storage-type enum |
| 4 | `offset` | `i32` frame-relative byte offset |
| 8 | `count` | `i32` element count |
| 12 | `countAsTime` | one-byte boolean; bytes 13–15 are padding |
| 16 | `name` | 32-byte NUL-terminated text field |
| 48 | `desc` | 64-byte NUL-terminated text field |
| 112 | `unit` | 32-byte NUL-terminated text field |

| Type value | SDK type | Bytes per element |
| ---: | --- | ---: |
| 0 | `char` | 1 |
| 1 | `bool` | 1 |
| 2 | `int` | 4 |
| 3 | `bitField` | 4 |
| 4 | `float` | 4 |
| 5 | `double` | 8 |

Value 6 (`ETCount`) is an enum bound, not a storage type. A parser MUST reject an unknown or sentinel type, negative offset, nonpositive count, or empty name. For each variable, `offset + count × elementWidth` MUST be checked for overflow and MUST NOT exceed `bufLen`. Arrays contain consecutive elements of the declared type. A name-keyed schema MUST reject duplicate names. Text ends at the first NUL within its fixed-width field; later bytes are not part of the value. `countAsTime` affects interpretation, not storage size.

## Session information

`sessionInfoLen` counts raw YAML bytes, not characters. A parser MUST use the advertised byte range to locate session information and MUST NOT search for a YAML document marker or NUL to determine the next region. The first NUL *within* that region, if present, ends its textual payload. `---` and `...` are not required for region discovery.

`WeekendInfo.Encoding` MAY declare `UTF8`, `UTF-8`, or `ISO_8859_1`. A session-text decoder MUST support UTF-8 and ISO-8859-1. Without a recognized declaration, it SHOULD try UTF-8 and fall back to ISO-8859-1 on invalid UTF-8. Decoding and YAML parsing occur after binary range validation; malformed text MUST NOT change the frame start. A parser MAY expose bounded raw session bytes when decoding or YAML parsing fails.

## Telemetry frames

Let `L = fileLength`, `F = frameStart`, and `B = bufLen`. Complete frame `i` occupies `[F + i × B, F + (i + 1) × B)`. Physical EOF is the only available telemetry-region end. A complete conforming recording has `L >= F` and `(L - F) mod B = 0`, with `(L - F) / B` frames. A nonzero remainder is a malformed or truncated final frame, not padding, and MUST NOT be silently treated as valid trailing data. A parser that accepts an incomplete file MAY expose preceding complete frames, but MUST distinguish the trailing bytes. Each frame is exactly `B` bytes and has no independent per-frame header. Variables are decoded at their declared frame-relative offsets using their declared storage types. A `SessionTime` variable, if present, is ordinary frame content.
