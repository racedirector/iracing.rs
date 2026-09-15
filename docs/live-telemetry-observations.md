# Live telemetry observations

This companion to [the live telemetry specification](live-telemetry-spec.md) records concrete repository and SDK observations. They are not additional protocol requirements. No raw mmap capture is checked into this repository, so this document does not claim a sample-by-sample memory geometry survey.

The local SDK 1.20 sample server reserves a 128 KiB session region, a maximum-size variable-header area, and three fixed-capacity frame buffers after the 112-byte header. It writes session storage first, then variable-header storage, then buffers. Offsets in the header remain authoritative. It initializes buffer tick fields to `-1`, fixes the variable list before setting the connected status, rotates buffers, and signals after each complete frame. Its test server uses 60 Hz; SDK prose describes triple buffering as giving consumers time to copy volatile data.

The generated [live variable schema](reference/live-variable-schema.yml) is one real discovery snapshot with a frame size of 8,587 bytes and 325 variables. It is representative of one car/session/build rather than a stable field catalog. The generated [live session schema](reference/live-session-schema.yml) contains an `ISO_8859_1` encoding example.

The current Rust header validation accepts 3–4 buffers, tick rates from 1 through 1,000, at most 5,000 variables, and frame sizes through 10,000,000 bytes. Those upper limits are defensive implementation limits, not constants defined by the wire protocol. The SDK 1.20 sample server itself declares a 4,096-variable capacity and uses three buffers.

The SDK sources in the local mirror use `curBuf` and the version-2 `tickCountBegin` torn-read check. An older [public SDK clone's utility implementation](https://github.com/vipoo/irsdk/blob/master/irsdk_utils.cpp) shows the earlier pattern of selecting the greatest tick and reading `tickCount` before and after the copy. The local version-2 producer comments and write sequence explicitly pair the pre-copy `tickCount` with the post-copy `tickCountBegin`; that newer local contract is the basis of the normative specification.
