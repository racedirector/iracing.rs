#!/usr/bin/env python3
"""Generate deterministic IBT test fixtures.

The generated layout intentionally follows the source-of-truth parser in
crates/iracing-sdk/src/ibt/format.rs and the common header shape in
crates/iracing-sdk/src/types/headers.rs:

- bytes 0..112: live-compatible irsdk_header prefix
- bytes 112..144: IBT disk sub-header
- bytes 144..: variable headers
"""

from __future__ import annotations

import hashlib
import json
import random
import struct
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
IBT_DIR = REPO_ROOT / "test-data" / "ibt"
YAML_DIR = REPO_ROOT / "test-data" / "session-yaml"
MANIFEST_PATH = IBT_DIR / "manifest.json"

IRSDK_HEADER_SIZE = 112
IRSDK_LIVE_HEADER_PREFIX_SIZE = IRSDK_HEADER_SIZE
IRSDK_DISK_SUBHEADER_SIZE = 32
IRSDK_IBT_HEADER_SIZE = IRSDK_HEADER_SIZE + IRSDK_DISK_SUBHEADER_SIZE
IRSDK_VAR_HEADER_SIZE = 144

VAR_TYPES = {
    "Char": 0,
    "Bool": 1,
    "Int32": 2,
    "BitField": 3,
    "Float32": 4,
    "Float64": 5,
}


@dataclass(frozen=True)
class Variable:
    name: str
    data_type: str
    offset: int
    count: int
    units: str
    description: str
    count_as_time: bool = False

    @property
    def size(self) -> int:
        scalar_sizes = {
            "Char": 1,
            "Bool": 1,
            "Int32": 4,
            "BitField": 4,
            "Float32": 4,
            "Float64": 8,
        }
        return scalar_sizes[self.data_type] * self.count


@dataclass(frozen=True)
class Profile:
    name: str
    seed: int
    track_name: str
    track_display_name: str
    session_name: str
    tick_rate: int
    frame_count: int
    frame_size: int
    lap_count: int
    start_date: int
    start_time: float
    variables: tuple[Variable, ...]


def base_variables() -> tuple[Variable, ...]:
    return (
        Variable("SessionTime", "Float64", 0, 1, "s", "Seconds since session start"),
        Variable("Speed", "Float32", 8, 1, "m/s", "Vehicle speed"),
        Variable("LapDist", "Float32", 12, 1, "m", "Distance around lap"),
        Variable("LapCompleted", "Int32", 16, 1, "", "Completed laps"),
        Variable("Brake", "Float32", 20, 1, "%", "Brake pedal input"),
        Variable("Throttle", "Float32", 24, 1, "%", "Throttle pedal input"),
        Variable("RPM", "Float32", 28, 1, "revs/min", "Engine speed"),
        Variable("Gear", "Int32", 32, 1, "", "Selected gear"),
    )


PROFILES = (
    Profile(
        name="profile_small",
        seed=10_001,
        track_name="generated small",
        track_display_name="Generated Small Circuit",
        session_name="Practice",
        tick_rate=60,
        frame_count=12,
        frame_size=48,
        lap_count=1,
        start_date=1_775_785_000,
        start_time=120.0,
        variables=base_variables(),
    ),
    Profile(
        name="profile_medium",
        seed=20_002,
        track_name="generated medium",
        track_display_name="Generated Medium Circuit",
        session_name="Qualify",
        tick_rate=60,
        frame_count=24,
        frame_size=64,
        lap_count=3,
        start_date=1_775_786_000,
        start_time=240.0,
        variables=base_variables()
        + (
            Variable("SteeringWheelAngle", "Float32", 36, 1, "rad", "Steering wheel angle"),
            Variable("FuelLevel", "Float32", 40, 1, "l", "Fuel level"),
        ),
    ),
    Profile(
        name="profile_large",
        seed=30_003,
        track_name="generated large",
        track_display_name="Generated Large Circuit",
        session_name="Race",
        tick_rate=60,
        frame_count=48,
        frame_size=96,
        lap_count=8,
        start_date=1_775_787_000,
        start_time=360.0,
        variables=base_variables()
        + (
            Variable("SteeringWheelAngle", "Float32", 36, 1, "rad", "Steering wheel angle"),
            Variable("FuelLevel", "Float32", 40, 1, "l", "Fuel level"),
            Variable("TrackTemp", "Float32", 44, 1, "C", "Track temperature"),
            Variable("OnPitRoad", "Bool", 48, 1, "", "Whether car is on pit road"),
            Variable("SessionFlags", "BitField", 52, 1, "", "Session status flags"),
        ),
    ),
)


def pack_i32(value: int) -> bytes:
    return struct.pack("<i", value)


def fixed_ascii(value: str, size: int) -> bytes:
    encoded = value.encode("ascii")
    if len(encoded) >= size:
        raise ValueError(f"{value!r} exceeds fixed field size {size}")
    return encoded + bytes(size - len(encoded))


def session_yaml(profile: Profile) -> str:
    return (
        "WeekendInfo:\n"
        f"  TrackName: {profile.track_name}\n"
        f"  TrackDisplayName: {profile.track_display_name}\n"
        '  TrackLength: "1.00 km"\n'
        "  TrackID: 9001\n"
        "SessionInfo:\n"
        "  CurrentSessionNum: 0\n"
        "  Sessions:\n"
        "    - SessionNum: 0\n"
        "      SessionLaps: unlimited\n"
        '      SessionTime: "600 sec"\n'
        f"      SessionType: {profile.session_name}\n"
        f"      SessionName: {profile.session_name}\n"
    )


def build_var_header(variable: Variable) -> bytes:
    header = bytearray(IRSDK_VAR_HEADER_SIZE)
    header[0:4] = pack_i32(VAR_TYPES[variable.data_type])
    header[4:8] = pack_i32(variable.offset)
    header[8:12] = pack_i32(variable.count)
    header[12] = 1 if variable.count_as_time else 0
    header[16:48] = fixed_ascii(variable.name, 32)
    header[48:112] = fixed_ascii(variable.description, 64)
    header[112:144] = fixed_ascii(variable.units, 32)
    return bytes(header)


def write_float32(frame: bytearray, offset: int, value: float) -> None:
    frame[offset : offset + 4] = struct.pack("<f", value)


def write_float64(frame: bytearray, offset: int, value: float) -> None:
    frame[offset : offset + 8] = struct.pack("<d", value)


def write_i32(frame: bytearray, offset: int, value: int) -> None:
    frame[offset : offset + 4] = pack_i32(value)


def build_frame(profile: Profile, frame_index: int, rng: random.Random) -> bytes:
    frame = bytearray(profile.frame_size)
    session_time = frame_index / profile.tick_rate
    write_float64(frame, 0, session_time)
    write_float32(frame, 8, 35.0 + frame_index * 0.25)
    write_float32(frame, 12, frame_index * 18.5)
    write_i32(frame, 16, frame_index // max(1, profile.frame_count // max(1, profile.lap_count)))
    write_float32(frame, 20, 0.15 + (frame_index % 4) * 0.1)
    write_float32(frame, 24, 0.55 + (frame_index % 5) * 0.05)
    write_float32(frame, 28, 3200.0 + frame_index * 12.0)
    write_i32(frame, 32, 1 + (frame_index % 5))

    if profile.frame_size >= 44:
        write_float32(frame, 36, -0.12 + rng.random() * 0.24)
        write_float32(frame, 40, 45.0 - frame_index * 0.02)
    if profile.frame_size >= 56:
        write_float32(frame, 44, 31.5 + frame_index * 0.01)
        frame[48] = 1 if frame_index in (0, profile.frame_count - 1) else 0
        write_i32(frame, 52, 0x1 if frame_index % 2 == 0 else 0x5)

    return bytes(frame)


def build_ibt(profile: Profile, yaml_bytes: bytes) -> tuple[bytes, int, int, int]:
    num_vars = len(profile.variables)
    var_header_offset = IRSDK_IBT_HEADER_SIZE
    var_headers_len = num_vars * IRSDK_VAR_HEADER_SIZE
    session_info_offset = var_header_offset + var_headers_len
    end_time = profile.start_time + (profile.frame_count / profile.tick_rate)

    header = bytearray(IRSDK_IBT_HEADER_SIZE)
    header[0:4] = pack_i32(2)
    header[4:8] = pack_i32(1)
    header[8:12] = pack_i32(profile.tick_rate)
    header[12:16] = pack_i32(0)
    header[16:20] = pack_i32(len(yaml_bytes))
    header[20:24] = pack_i32(session_info_offset)
    header[24:28] = pack_i32(num_vars)
    header[28:32] = pack_i32(var_header_offset)
    header[32:36] = pack_i32(1)
    header[36:40] = pack_i32(profile.frame_size)

    # IBT disk sub-header occupies bytes 112..144, immediately before var headers.
    disk_offset = IRSDK_LIVE_HEADER_PREFIX_SIZE
    header[disk_offset : disk_offset + 8] = struct.pack("<q", profile.start_date)
    header[disk_offset + 8 : disk_offset + 16] = struct.pack("<d", profile.start_time)
    header[disk_offset + 16 : disk_offset + 24] = struct.pack("<d", end_time)
    header[disk_offset + 24 : disk_offset + 28] = pack_i32(profile.lap_count)
    header[disk_offset + 28 : disk_offset + 32] = pack_i32(profile.frame_count)

    var_headers = b"".join(build_var_header(variable) for variable in profile.variables)
    rng = random.Random(profile.seed)
    frames = b"".join(build_frame(profile, index, rng) for index in range(profile.frame_count))
    ibt_bytes = bytes(header) + var_headers + yaml_bytes + frames
    return ibt_bytes, var_header_offset, disk_offset, session_info_offset


def manifest_variable(variable: Variable) -> dict[str, object]:
    return {
        "name": variable.name,
        "data_type": variable.data_type,
        "offset": variable.offset,
        "count": variable.count,
        "units": variable.units,
    }


def main() -> None:
    IBT_DIR.mkdir(parents=True, exist_ok=True)
    YAML_DIR.mkdir(parents=True, exist_ok=True)

    fixtures = []
    for profile in PROFILES:
        yaml_text = session_yaml(profile)
        yaml_bytes = yaml_text.encode("utf-8")
        yaml_path = YAML_DIR / f"{profile.name}.yaml"
        ibt_path = IBT_DIR / f"{profile.name}.ibt"

        yaml_path.write_bytes(yaml_bytes)
        ibt_bytes, var_header_offset, disk_offset, session_info_offset = build_ibt(
            profile, yaml_bytes
        )
        ibt_path.write_bytes(ibt_bytes)

        fixtures.append(
            {
                "name": profile.name,
                "path": f"test-data/ibt/{profile.name}.ibt",
                "session_yaml_path": f"test-data/session-yaml/{profile.name}.yaml",
                "seed": profile.seed,
                "tick_rate": profile.tick_rate,
                "num_vars": len(profile.variables),
                "frame_size": profile.frame_size,
                "num_frames": profile.frame_count,
                "var_header_offset": var_header_offset,
                "disk_sub_header_offset": disk_offset,
                "session_info_update": 0,
                "session_info_len": len(yaml_bytes),
                "session_info_offset": session_info_offset,
                "num_buf": 1,
                "disk_header": {
                    "start_date": profile.start_date,
                    "start_time": profile.start_time,
                    "end_time": profile.start_time + (profile.frame_count / profile.tick_rate),
                    "lap_count": profile.lap_count,
                    "record_count": profile.frame_count,
                },
                "sha256": hashlib.sha256(ibt_bytes).hexdigest(),
                "required_variables": [manifest_variable(variable) for variable in profile.variables],
            }
        )

    manifest = {
        "schema_version": 1,
        "generated_by": "scripts/generate_test_fixtures.py",
        "layout": {
            "live_header_prefix_size": IRSDK_LIVE_HEADER_PREFIX_SIZE,
            "ibt_header_size": IRSDK_IBT_HEADER_SIZE,
            "disk_sub_header_size": IRSDK_DISK_SUBHEADER_SIZE,
            "variable_header_size": IRSDK_VAR_HEADER_SIZE,
            "disk_sub_header_offset_rule": "var_header_offset - disk_sub_header_size",
        },
        "fixtures": fixtures,
    }
    MANIFEST_PATH.write_bytes((json.dumps(manifest, indent=2) + "\n").encode("utf-8"))


if __name__ == "__main__":
    main()
