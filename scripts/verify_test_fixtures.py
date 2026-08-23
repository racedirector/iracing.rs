#!/usr/bin/env python3
"""Verify generated IBT fixtures against their manifest."""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = REPO_ROOT / "test-data" / "ibt" / "manifest.json"


def read_i32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def read_i64(data: bytes, offset: int) -> int:
    return struct.unpack_from("<q", data, offset)[0]


def read_f64(data: bytes, offset: int) -> float:
    return struct.unpack_from("<d", data, offset)[0]


def fail(message: str) -> None:
    print(f"fixture verification failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def verify_fixture(fixture: dict[str, object], layout: dict[str, object]) -> None:
    relative_path = str(fixture["path"])
    path = REPO_ROOT / relative_path
    if not path.is_file():
        fail(f"missing fixture {relative_path}")

    data = path.read_bytes()
    expected_sha = str(fixture["sha256"])
    actual_sha = hashlib.sha256(data).hexdigest()
    if actual_sha != expected_sha:
        fail(f"{relative_path} sha256 mismatch: {actual_sha} != {expected_sha}")

    ibt_header_size = int(layout["ibt_header_size"])
    live_header_size = int(layout["live_header_prefix_size"])
    disk_size = int(layout["disk_sub_header_size"])
    if ibt_header_size != live_header_size + disk_size:
        fail(
            "IBT header size does not equal the live header prefix plus disk sub-header size"
        )
    if len(data) < ibt_header_size:
        fail(f"{relative_path} is shorter than IBT header size")

    checks = {
        "version": (read_i32(data, 0), 2),
        "status": (read_i32(data, 4), 1),
        "tick_rate": (read_i32(data, 8), int(fixture["tick_rate"])),
        "session_info_update": (read_i32(data, 12), int(fixture["session_info_update"])),
        "session_info_len": (read_i32(data, 16), int(fixture["session_info_len"])),
        "session_info_offset": (read_i32(data, 20), int(fixture["session_info_offset"])),
        "num_vars": (read_i32(data, 24), int(fixture["num_vars"])),
        "var_header_offset": (read_i32(data, 28), int(fixture["var_header_offset"])),
        "num_buf": (read_i32(data, 32), int(fixture["num_buf"])),
        "frame_size": (read_i32(data, 36), int(fixture["frame_size"])),
    }
    for label, (actual, expected) in checks.items():
        if actual != expected:
            fail(f"{relative_path} {label} mismatch: {actual} != {expected}")

    disk_offset = int(fixture["var_header_offset"]) - disk_size
    if int(fixture["var_header_offset"]) != ibt_header_size:
        fail(f"{relative_path} variable headers do not immediately follow the IBT header")
    if disk_offset != int(fixture["disk_sub_header_offset"]):
        fail(f"{relative_path} disk offset does not follow var_header_offset - disk_size")

    disk_header = fixture["disk_header"]
    assert isinstance(disk_header, dict)
    if read_i64(data, disk_offset) != int(disk_header["start_date"]):
        fail(f"{relative_path} disk start_date mismatch")
    if abs(read_f64(data, disk_offset + 8) - float(disk_header["start_time"])) > 1e-9:
        fail(f"{relative_path} disk start_time mismatch")
    if abs(read_f64(data, disk_offset + 16) - float(disk_header["end_time"])) > 1e-9:
        fail(f"{relative_path} disk end_time mismatch")
    if read_i32(data, disk_offset + 24) != int(disk_header["lap_count"]):
        fail(f"{relative_path} disk lap_count mismatch")
    if read_i32(data, disk_offset + 28) != int(disk_header["record_count"]):
        fail(f"{relative_path} disk record_count mismatch")

    var_headers_end = int(fixture["var_header_offset"]) + int(fixture["num_vars"]) * int(
        layout["variable_header_size"]
    )
    session_end = int(fixture["session_info_offset"]) + int(fixture["session_info_len"])
    expected_len = session_end + int(fixture["num_frames"]) * int(fixture["frame_size"])
    if var_headers_end != int(fixture["session_info_offset"]):
        fail(f"{relative_path} session info does not immediately follow variable headers")
    if len(data) != expected_len:
        fail(f"{relative_path} length mismatch: {len(data)} != {expected_len}")

    yaml_path = REPO_ROOT / str(fixture["session_yaml_path"])
    if not yaml_path.is_file():
        fail(f"missing session YAML {fixture['session_yaml_path']}")
    yaml_bytes = yaml_path.read_bytes()
    embedded_yaml = data[int(fixture["session_info_offset"]) : session_end]
    if embedded_yaml != yaml_bytes:
        fail(f"{relative_path} embedded YAML does not match {fixture['session_yaml_path']}")


def main() -> None:
    if not MANIFEST_PATH.is_file():
        fail(f"missing manifest {MANIFEST_PATH.relative_to(REPO_ROOT)}")

    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != 1:
        fail("unsupported manifest schema_version")

    layout = manifest["layout"]
    fixtures = manifest["fixtures"]
    if len(fixtures) < 3:
        fail("expected at least three fixtures")

    for fixture in fixtures:
        verify_fixture(fixture, layout)

    print(f"verified {len(fixtures)} generated IBT fixtures")


if __name__ == "__main__":
    main()
