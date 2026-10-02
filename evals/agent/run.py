#!/usr/bin/env python3
"""Opt-in corpus preparation and proposition-level manual adjudication."""
import sys
from types import SimpleNamespace
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from datetime import datetime, timezone

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def cases(here=HERE):
    result = []
    for file in sorted((here / "fixtures").glob("*/case.json")):
        case = read(file)
        if set(case) != {"id", "prompt", "inputs"} or case["id"] != file.parent.name:
            raise ValueError(f"invalid treatment metadata: {file}")
        if not case["prompt"] or not case["inputs"]:
            raise ValueError(f"empty treatment: {file}")
        for item in case["inputs"]:
            path = file.parent / item
            try:
                resolved = path.resolve(strict=True)
            except FileNotFoundError as error:
                raise ValueError(f"{case['id']}: missing input {item}") from error
            if not resolved.is_relative_to(file.parent.resolve()) or not resolved.is_file():
                raise ValueError(f"{case['id']}: input outside treatment: {item}")
        expected = {"case.json", *case["inputs"]}
        actual = {str(p.relative_to(file.parent)) for p in file.parent.rglob("*") if p.is_file()}
        if expected != actual:
            raise ValueError(f"unregistered treatment files: {file}")
        oracle = read(here / "oracles" / (case["id"] + ".json"))
        if oracle["id"] != case["id"] or not oracle["atoms"] or not oracle["hard_failures"]:
            raise ValueError(f"invalid oracle: {file}")
        result.append((case, oracle, file.parent))
    if len(result) != 8 or not any(o["control"] for _, o, _ in result):
        raise ValueError("require eight cases including a valid control")
    if {p.stem for p in (here / "oracles").glob("*.json")} != {c["id"] for c, _, _ in result}:
        raise ValueError("orphaned/missing oracle")
    return result


def digest(files):
    value = hashlib.sha256()
    for name, data in sorted(files.items()):
        value.update(name.encode() + b"\0" + data + b"\0")
    return value.hexdigest()


def tree_digest(path):
    return digest({str(p.relative_to(path)): p.read_bytes()
                   for p in path.rglob("*") if p.is_file()})


def frozen(condition, path=None):
    if path is None:
        path = HERE / "frozen-skills" / condition
    metadata = read(path / "manifest.json")
    files = {str(p.relative_to(path)): p.read_bytes() for p in path.rglob("*") if p.is_file() and p.name != "manifest.json"}
    if digest(files) != condition or metadata["hash"] != condition:
        raise ValueError("frozen skill content was modified")
    if metadata["files"] != sorted(files):
        raise ValueError("frozen package inventory mismatch")
    return path


def freeze():
    files = {}
    for name in ("rust-soundness-review", "rust-unsafe-ffi"):
        source = ROOT / ".agents/skills" / name
        for p in source.rglob("*"):
            if p.is_file():
                files[str(p.relative_to(ROOT))] = p.read_bytes()
    # Include boundary references actually routed by the skills.
    for name in ("docs/architecture/unsafe-boundaries.md", "docs/architecture/platform-and-features.md"):
        files[name] = (ROOT / name).read_bytes()
    key = digest(files)
    destination = HERE / "frozen-skills" / key
    if destination.exists():
        frozen(key)
    else:
        for name, data in files.items():
            p = destination / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
        write(destination / "manifest.json", {"hash": key, "files": sorted(files)})
    return key


def prepare(args):
    suite = cases()
    if args.condition != "baseline":
        package = frozen(args.condition)
    run = args.run
    run.mkdir(parents=True, exist_ok=False)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    write(run / "metadata.json", {"model": args.model, "tools": args.tools, "condition": args.condition,
        "skill_revision": None if args.condition == "baseline" else args.condition,
        "corpus_digest": tree_digest(HERE / "fixtures"),
        "oracle_digest": tree_digest(HERE / "oracles"),
        "repository_revision": revision, "timestamp": datetime.now(timezone.utc).isoformat()})
    judgments = {}
    for case, oracle, directory in suite:
        shutil.copytree(directory, run / "treatments" / case["id"])
        judgments[case["id"]] = {
            "atoms": {key: {"pass": None, "evidence": ""} for key in oracle["atoms"]},
            "hard_failures": {key: {"triggered": None, "evidence": ""} for key in oracle["hard_failures"]}}
    if args.condition != "baseline":
        shutil.copytree(package, run / "skills")
    (run / "responses").mkdir()
    write(run / "judgments.json", judgments)


def score(run):
    metadata = read(run / "metadata.json")
    if not all(metadata.get(key) for key in ("model", "tools", "condition", "repository_revision", "timestamp")):
        raise ValueError("incomplete run provenance")
    if metadata["condition"] != "baseline":
        frozen(metadata["condition"], run / "skills")
    if (metadata.get("corpus_digest") != tree_digest(HERE / "fixtures")
            or metadata.get("corpus_digest") != tree_digest(run / "treatments")):
        raise ValueError("run corpus digest differs")
    if metadata.get("oracle_digest") != tree_digest(HERE / "oracles"):
        raise ValueError("run oracle digest differs")
    judgments = read(run / "judgments.json")
    report = {}
    for case, oracle, _ in cases():
        ident = case["id"]
        if not (run / "responses" / (ident + ".md")).read_text().strip():
            raise ValueError(f"missing response: {ident}")
        value = judgments[ident]
        for field, flag in (("atoms", "pass"), ("hard_failures", "triggered")):
            if set(value[field]) != set(oracle[field]):
                raise ValueError(f"incomplete judgments: {ident}/{field}")
            for judgment in value[field].values():
                if type(judgment[flag]) is not bool or not judgment["evidence"].strip():
                    raise ValueError(f"unadjudicated proposition: {ident}/{field}")
        passed = sum(v["pass"] for v in value["atoms"].values())
        hard = any(v["triggered"] for v in value["hard_failures"].values())
        report[ident] = {"atoms_passed": passed, "atoms_total": len(oracle["atoms"]),
            "hard_failure": hard, "pass": passed == len(oracle["atoms"]) and not hard}
    result = {"metadata": metadata, "cases": report}
    write(run / "score.json", result)
    return result


def compare(left_run, right_run):
    left, right = score(left_run), score(right_run)
    for field in ("model", "tools"):
        if left["metadata"][field] != right["metadata"][field]:
            raise ValueError(f"cannot compare runs with different {field}")
    return {"left": left["metadata"], "right": right["metadata"], "changes": {
        key: {"left": left["cases"][key], "right": right["cases"][key]}
        for key in left["cases"]}}


def main():
    # Internal JSON protocol: the public CLI is cargo xtask agent (clap).
    request = json.load(sys.stdin)
    for name in ("run", "left", "right"):
        if name in request:
            request[name] = Path(request[name])
    args = SimpleNamespace(**request)
    if args.command == "check":
        cases()
        for directory in (HERE / "frozen-skills").iterdir():
            frozen(directory.name)
        print("Corpus and frozen package integrity passed")
    elif args.command == "freeze":
        print(freeze())
    elif args.command == "prepare":
        prepare(args)
    elif args.command == "score":
        print(json.dumps(score(args.run), indent=2))
    elif args.command == "compare":
        print(json.dumps(compare(args.left, args.right), indent=2))

    else:
        raise ValueError(f"unknown backend command: {args.command}")


if __name__ == "__main__":
    main()
