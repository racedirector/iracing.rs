#!/usr/bin/env python3
"""Regenerate and verify deterministic IBT fixtures.

This is the single entrypoint for humans, agents, and CI. It runs the generator,
verifies the manifest and fixture bytes, and optionally checks for git drift.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=REPO_ROOT, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--no-drift-check",
        action="store_true",
        help="Skip the git diff check after regeneration.",
    )
    args = parser.parse_args()

    python = sys.executable
    run([python, "scripts/generate_test_fixtures.py"])
    run([python, "scripts/verify_test_fixtures.py"])

    if not args.no_drift_check:
        run(["git", "diff", "--exit-code", "--", "test-data/ibt", "test-data/session-yaml"])


if __name__ == "__main__":
    main()
