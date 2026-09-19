# IBT capture observations

This companion to [the IBT specification](ibt-spec.md) records evidence from the nine `.ibt` files under `test-data`: six real captures and three deterministic fixtures. These findings are not additional format requirements. After reviewing `crates/iracing-sdk/src/bin/headers.rs` and the other available binaries, the geometry was checked from the file bytes. `V end = varHeaderOffset + numVars × 144`; `S end = sessionInfoOffset + sessionInfoLen`.

| File | Variables | V end = S start | S end = frame start | Frame bytes | Frames |
| --- | ---: | ---: | ---: | ---: | ---: |
| Donington 21:42:08 | 287 | 41,472 | 67,788 | 1,107 | 54,574 |
| Donington 21:57:46 | 287 | 41,472 | 71,516 | 1,107 | 5,306 |
| Donington 21:59:18 | 287 | 41,472 | 71,517 | 1,107 | 31,374 |
| Donington 22:08:49 | 287 | 41,472 | 75,861 | 1,107 | 48,233 |
| Bristol race | 274 | 39,600 | 103,888 | 1,070 | 133,169 |
| Interlagos 11:24:51 | 287 | 41,472 | 85,814 | 1,107 | 56,117 |
| `profile_small` | 8 | 1,296 | 1,605 | 48 | 12 |
| `profile_medium` | 10 | 1,584 | 1,893 | 64 | 24 |
| `profile_large` | 13 | 2,016 | 2,317 | 96 | 48 |

All nine have SDK version 2, `tickRate = 60`, `status = 1`, `numBuf = 1`, `sessionInfoUpdate = 0`, and `varHeaderOffset = 144`. Variable headers precede session text without a gap; frames start at the latest metadata end (the end of session text in every capture). Each frame region has no partial tail, and its EOF-derived complete-frame count equals `sessionRecordCount`. These files therefore corroborate the compatibility rule but do not prove all legal region orderings. They do not exercise session-before-variables, gaps, absent regions, partial frames, or mismatched record counts.

The six real captures set `varBuf[0].bufOffset` to the computed frame start. The generated fixtures set it to zero. Real `varBuf[0].tickCount` values do not equal final record counts. Five real captures have non-UTF-8 session bytes and no `Encoding` declaration. Interlagos declares `Encoding: UTF8` and is valid UTF-8. The generated sessions are ASCII without an encoding declaration. Real sessions start with `---` and end with `...\n`; generated ones omit those markers.

The local [SDK disk writer](../../irsdk_1_20/irsdk_diskclient.cpp) places the main header, disk sub-header, variable headers, session text, and frames contiguously in that order. [The fixture generator](../crates/test-fixtures/src/generate.rs) follows that order but zeroes buffer descriptors. Its `sessionEndTime = sessionStartTime + frameCount / tickRate` is a fixture convention, not an observed requirement for real captures.
