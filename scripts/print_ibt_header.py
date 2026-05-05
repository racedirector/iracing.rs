#!/usr/bin/env python3
"""Print header information from an iRacing .ibt file using pyirsdk."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

try:
    import irsdk
except ImportError as exc:
    print(
        "pyirsdk is required. Install it with: pip install pyirsdk",
        file=sys.stderr,
    )
    raise SystemExit(1) from exc


def _property_names(obj: object) -> list[str]:
    return sorted(
        name
        for name, value in vars(type(obj)).items()
        if isinstance(value, property) and not name.startswith("_")
    )


def _print_section(title: str, obj: object) -> None:
    print(title)
    for name in _property_names(obj):
        value = getattr(obj, name)
        print(f"  {name}: {value}")
    print()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Open an iRacing .ibt file and print its header information."
    )
    parser.add_argument("ibt_file", type=Path, help="Path to the .ibt file")
    args = parser.parse_args()

    if not args.ibt_file.is_file():
        print(f"File not found: {args.ibt_file}", file=sys.stderr)
        return 1

    ibt = irsdk.IBT()
    try:
        ibt.open(str(args.ibt_file))

        print(f"File: {args.ibt_file.resolve()}")
        print()
        _print_section("Main Header", ibt._header)
        _print_section("Disk Subheader", ibt._disk_header)
        print(f"Variable headers: {ibt._header.num_vars}")
        print(f"Variable buffer tick: {ibt.var_header_buffer_tick}")
        print(f"Available variables: {len(ibt.var_headers_names or [])}")
        return 0
    finally:
        ibt.close()


if __name__ == "__main__":
    raise SystemExit(main())
