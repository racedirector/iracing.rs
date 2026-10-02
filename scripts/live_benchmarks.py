"""Persist, compare, and promote simulator-backed live benchmark summaries."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STORE = Path(os.environ.get("IRACING_LIVE_BENCH_STORE", ROOT / ".live-benchmarks"))
RESULTS = ROOT / "docs" / "benchmarks" / "results" / "live"
LABEL = re.compile(r"[A-Za-z0-9_-]{1,48}\Z")
PERCENTILES = ("p50_us", "p95_us", "p99_us")
EXPECTED_CASES = {
    "live_acquisition/owned_frame",
    "live_consumer/dynamic_1",
    "live_consumer/dynamic_4",
}
ACQUISITION_METRICS = set(PERCENTILES) | {
    "attempted", "accepted_total", "accepted_frames", "no_frame_polls", "wait_signals",
    "wait_timeouts", "copied_bytes", "elapsed_s", "effective_hz",
    "skipped_ticks", "failure", "samples_ns",
}
CONSUMER_METRICS = set(PERCENTILES) | {
    "source_tick_opportunities", "elapsed_s", "subscriber_count_skew",
    "subscribers", "failure",
}
SUBSCRIBER_METRICS = {"index", "received", "skipped_or_coalesced_ticks",
                      "p50_us", "p95_us", "p99_us", "effective_hz", "intervals_ns"}


def load(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    validate(data)
    return data


def validate(data: dict) -> None:
    if data.get("format_version") != 1:
        raise ValueError("unsupported live result format")
    if not isinstance(data.get("run_id"), str) or not LABEL.fullmatch(data["run_id"]):
        raise ValueError("invalid run ID")
    if not isinstance(data.get("profile"), str) or not LABEL.fullmatch(data["profile"]):
        raise ValueError("invalid profile label")
    if not re.fullmatch(r"[0-9a-f]{40}", data.get("git_sha", "")):
        raise ValueError("missing full commit SHA")
    if type(data.get("git_dirty")) is not bool:
        raise ValueError("missing dirty state")
    env = data.get("environment")
    scenario = data.get("scenario")
    if not isinstance(env, dict) or not isinstance(scenario, dict):
        raise ValueError("missing environment or scenario")
    for key in ("os_version", "cpu_model", "logical_cores", "rustc", "target", "build_profile", "power_profile"):
        if key not in env:
            raise ValueError(f"missing environment field: {key}")
    for key in ("source", "declared_tick_hz", "frame_size", "schema_fingerprint", "warmup_frames", "target_frames", "workload_version"):
        if key not in scenario:
            raise ValueError(f"missing scenario field: {key}")
    if scenario["source"] != "active-iracing" or scenario["target_frames"] < 600:
        raise ValueError("invalid live scenario")
    if not isinstance(data.get("cases"), list) or not data["cases"]:
        raise ValueError("missing cases")
    seen = set()
    for case in data["cases"]:
        if case.get("id") not in EXPECTED_CASES or case["id"] in seen:
            raise ValueError("unknown or duplicate case")
        seen.add(case["id"])
        if (not isinstance(case.get("experiment_version"), int)
                or case["experiment_version"] < 1
                or case.get("status") not in {"complete", "incomplete", "skipped"}):
            raise ValueError("invalid experiment or case status")
        if not isinstance(case.get("parameters"), dict) or not isinstance(case.get("metrics"), dict):
            raise ValueError("missing case parameters or metrics")
        expected_parameters = (
            {"timed_boundary": "get_new_data+to_vec+provider_metadata"}
            if case["id"] == "live_acquisition/owned_frame"
            else {"subscribers": 1 if case["id"].endswith("dynamic_1") else 4,
                  "adapter": "DynamicFrame", "delivery": "latest-wins"}
        )
        if case["parameters"] != expected_parameters:
            raise ValueError("unexpected case parameters")
        allowed = ACQUISITION_METRICS if case["id"].startswith("live_acquisition/") else CONSUMER_METRICS
        if set(case["metrics"]) - allowed:
            raise ValueError("unknown metrics could contain live telemetry")
        if case["id"].startswith("live_consumer/"):
            if not isinstance(case["metrics"].get("subscribers"), list):
                raise ValueError("missing subscriber observations")
            if any(set(s) - SUBSCRIBER_METRICS for s in case["metrics"]["subscribers"]):
                raise ValueError("unknown subscriber metrics could contain live telemetry")
        if case["status"] == "complete":
            if case.get("samples", 0) < 600:
                raise ValueError("complete case has fewer than 600 samples")
            if any(not isinstance(case["metrics"].get(k), (int, float)) for k in PERCENTILES):
                raise ValueError("complete case lacks percentile measurements")


def retained_path(run: dict) -> Path:
    return STORE / "runs" / f"{run['run_id']}.json"


def record(args: argparse.Namespace) -> None:
    run = load(args.input)
    destination = retained_path(run)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        raise ValueError(f"run already recorded: {destination}")
    shutil.copyfile(args.input, destination)
    if args.label:
        if not LABEL.fullmatch(args.label):
            raise ValueError("invalid baseline label")
        if run["git_dirty"] or any(c["status"] != "complete" for c in run["cases"]):
            raise ValueError("baseline label requires a clean, complete run")
        labels_path = STORE / "baselines.json"
        labels = json.loads(labels_path.read_text(encoding="utf-8")) if labels_path.exists() else {}
        if args.label in labels:
            raise ValueError(f"baseline label already exists: {args.label}")
        labels[args.label] = {"run_id": run["run_id"], "git_sha": run["git_sha"]}
        labels_path.write_text(json.dumps(labels, indent=2) + "\n", encoding="utf-8")
    print(destination)


def resolve_baseline(label: str) -> Path:
    labels = json.loads((STORE / "baselines.json").read_text(encoding="utf-8"))
    entry = labels[label]
    path = STORE / "runs" / f"{entry['run_id']}.json"
    run = load(path)
    if run["git_sha"] != entry["git_sha"]:
        raise ValueError("baseline manifest SHA does not match its run")
    return path


def compare_runs(base: dict, head: dict) -> dict:
    env_keys = ("os_version", "cpu_model", "logical_cores", "rustc", "target", "build_profile", "power_profile")
    scenario_keys = ("source", "declared_tick_hz", "frame_size", "schema_fingerprint",
                     "warmup_frames", "target_frames", "workload_version")
    reasons = []
    if base["profile"] != head["profile"]:
        reasons.append("machine profile")
    reasons += [f"environment.{key}" for key in env_keys if base["environment"][key] != head["environment"][key]]
    reasons += [f"scenario.{key}" for key in scenario_keys if base["scenario"][key] != head["scenario"][key]]
    if base["git_dirty"] or head["git_dirty"]:
        reasons.append("dirty checkout")
    base_cases = {case["id"]: case for case in base["cases"]}
    head_cases = {case["id"]: case for case in head["cases"]}
    cases = []
    for case_id in sorted(base_cases.keys() | head_cases.keys()):
        left, right = base_cases.get(case_id), head_cases.get(case_id)
        if left is None or right is None:
            cases.append({"id": case_id, "status": "unpaired"})
            continue
        local_reasons = reasons.copy()
        if left["experiment_version"] != right["experiment_version"]:
            cases.append({"id": case_id, "status": "unpaired", "reasons": ["experiment version"]})
            continue
        if left["parameters"] != right["parameters"]:
            local_reasons.append("case parameters")
        if left["status"] != "complete" or right["status"] != "complete":
            local_reasons.append("incomplete case")
        if local_reasons:
            cases.append({"id": case_id, "status": "incomparable", "reasons": local_reasons})
            continue
        metrics = {}
        for name in PERCENTILES:
            before, after = left["metrics"][name], right["metrics"][name]
            metrics[name] = {"base_us": before, "head_us": after,
                             "change_pct": ((after - before) / before * 100) if before else None}
        cases.append({"id": case_id, "status": "diagnostic" if base["scenario"].get("conditions") != head["scenario"].get("conditions") else "comparable", "metrics": metrics})
    return {"format_version": 1, "base_run_id": base["run_id"], "base_git_sha": base["git_sha"],
            "head_run_id": head["run_id"], "head_git_sha": head["git_sha"], "cases": cases}


def markdown(delta: dict) -> str:
    lines = [f"Base `{delta['base_run_id']}` ({delta['base_git_sha']}) → head `{delta['head_run_id']}` ({delta['head_git_sha']})",
             "", "| Case | Status | p50 base → head (change) | p95 base → head (change) | p99 base → head (change) |",
             "| --- | --- | --- | --- | --- |"]
    for case in delta["cases"]:
        cells = []
        for name in PERCENTILES:
            value = case.get("metrics", {}).get(name)
            cells.append(f"{value['base_us']:.2f} → {value['head_us']:.2f} µs ({value['change_pct']:+.1f}%)" if value and value["change_pct"] is not None else "—")
        status = case["status"] + (": " + ", ".join(case["reasons"]) if case.get("reasons") else "")
        lines.append(f"| `{case['id']}` | {status} | {' | '.join(cells)} |")
    return "\n".join(lines) + "\n"


def compare(args: argparse.Namespace) -> None:
    base_path = resolve_baseline(args.baseline) if args.baseline else args.base
    delta = compare_runs(load(base_path), load(args.head))
    text = markdown(delta)
    print(text)
    if args.json_output:
        args.json_output.write_text(json.dumps(delta, indent=2) + "\n", encoding="utf-8")
    if args.markdown_output:
        args.markdown_output.write_text(text, encoding="utf-8")


def promote(args: argparse.Namespace) -> None:
    run = load(args.input)
    if run["git_dirty"] or any(c["status"] != "complete" for c in run["cases"]):
        raise ValueError("promotion requires a clean, complete run")
    # Explicit field allowlist prevents accidental telemetry or identity publication.
    safe = {key: run[key] for key in ("format_version", "run_id", "git_sha", "git_dirty", "profile")}
    safe["environment"] = {key: run["environment"][key] for key in
                           ("os_version", "cpu_model", "logical_cores", "rustc", "target", "build_profile", "power_profile")}
    safe["scenario"] = {key: run["scenario"][key] for key in
                        ("source", "declared_tick_hz", "frame_size", "schema_fingerprint", "warmup_frames", "target_frames", "workload_version")}
    safe["cases"] = []
    for case in run["cases"]:
        metrics = {key: value for key, value in case["metrics"].items() if key != "samples_ns"}
        if "subscribers" in metrics:
            metrics["subscribers"] = [
                {key: value for key, value in subscriber.items() if key != "intervals_ns"}
                for subscriber in metrics["subscribers"]
            ]
        safe["cases"].append({"id": case["id"], "experiment_version": case["experiment_version"],
                              "status": case["status"], "samples": case["samples"],
                              "parameters": case["parameters"], "metrics": metrics})
    destination = RESULTS / run["profile"] / f"{run['run_id']}.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("x", encoding="utf-8") as output:
        json.dump(safe, output, indent=2)
        output.write("\n")
    print(destination)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    record_parser = sub.add_parser("record")
    record_parser.add_argument("input", type=Path)
    record_parser.add_argument("--label")
    record_parser.set_defaults(func=record)
    compare_parser = sub.add_parser("compare")
    source = compare_parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--base", type=Path)
    source.add_argument("--baseline")
    compare_parser.add_argument("--head", type=Path, required=True)
    compare_parser.add_argument("--json-output", type=Path)
    compare_parser.add_argument("--markdown-output", type=Path)
    compare_parser.set_defaults(func=compare)
    promote_parser = sub.add_parser("promote")
    promote_parser.add_argument("input", type=Path)
    promote_parser.set_defaults(func=promote)
    args = parser.parse_args()
    try:
        args.func(args)
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
        parser.exit(2, f"live benchmark: {error}\n")


if __name__ == "__main__":
    main()
