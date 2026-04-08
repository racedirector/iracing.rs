# Phase 1 IBT lap indexing variable selection

This phase-1 lap index is intentionally limited to the player car and uses only fields verified in this repository's schemas.

## Selected variables

- `LapCompleted` *(player-scoped telemetry)* is the primary lap-boundary signal because `live-variable-schema.yml` defines it as the player's completed-lap counter.
- `Lap` *(player-scoped telemetry)* is recorded as the lap number / laps-started value at each lap start.
- `LapDistPct` *(player-scoped telemetry)* is the fallback lap-progress signal because the schema includes a scalar player value; when `LapCompleted` stays flat, a wrap from high progress to low progress marks a new lap slice.
- `OnPitRoad` *(player-scoped telemetry)* marks whether the player touched pit road during the indexed lap.
- `SessionTime` *(player-scoped telemetry)* supplies start/end timestamps for each frame range.
- `DriverInfo.DriverCarIdx` *(session YAML)* is still resolved from `live-session-schema.yml` / `session-schema.yml` so the helper can fall back to `CarIdxLapCompleted`, `CarIdxLap`, `CarIdxLapDistPct`, and `CarIdxOnPitRoad` if a file schema omits a player-scoped field.

## Rejected alternatives

- `CarIdxLapCompleted[player]` was not chosen as the primary boundary source because the repo schema already exposes the simpler player-scoped `LapCompleted`.
- `CarIdxLapDistPct[player]` was not chosen as the primary progress source because the repo schema already exposes player-scoped `LapDistPct`.
- `CarIdxOnPitRoad[player]` was not chosen as the primary pit signal because the repo schema already exposes player-scoped `OnPitRoad`.
- Sector boundaries from `SplitTimeInfo.Sectors[].SectorStartPct` were left out because phase 1 only indexes whole laps.

## Scope limits

- The index stores frame ranges only; it does not cache decoded telemetry frames.
- Only the player car is indexed in phase 1.
- Lap boundaries are detected from completed-lap increments first, then progress wrap as a fallback.
