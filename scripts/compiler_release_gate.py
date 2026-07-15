#!/usr/bin/env python3
"""Record a small, reproducible compiler release-gate baseline."""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def run_timed(command: list[str], timeout_seconds: float = 30.0) -> float:
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
    elapsed = (time.perf_counter() - started) * 1000.0
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{completed.stdout}{completed.stderr}"
        )
    return round(elapsed, 3)


def percentile_95(values: list[float]) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int(len(ordered) * 0.95 + 0.999) - 1))
    return ordered[index]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nomo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument(
        "--thresholds",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "performance"
        / "release-gate-thresholds.json",
    )
    args = parser.parse_args()
    if args.iterations < 3:
        parser.error("--iterations must be at least 3")
    executable = args.nomo.resolve()
    thresholds = json.loads(args.thresholds.read_text(encoding="utf-8"))

    with tempfile.TemporaryDirectory(prefix="nomo-compiler-release-gate-") as temporary:
        project = Path(temporary) / "app"
        source = project / "src" / "main.nomo"
        source.parent.mkdir(parents=True)
        (project / "nomo.toml").write_text(
            '[package]\nnamespace = "release-gate"\nname = "compiler"\n'
            'version = "0.0.0-20260713145859"\nedition = "2026"\n',
            encoding="utf-8",
        )
        source.write_text(
            "package app.main\n\n"
            "fn add(a: i64, b: i64) -> i64 {\n"
            "    return a + b\n"
            "}\n\n"
            "fn main() -> void {\n"
            "    let total: i64 = add(40, 2)\n"
            "}\n",
            encoding="utf-8",
        )

        subprocess.run(
            [str(executable), "clean", str(project)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        clean_build_ms = run_timed([str(executable), "build", str(project)])
        check_samples_ms = [
            run_timed([str(executable), "check", str(project)])
            for _ in range(args.iterations)
        ]
        help_output = subprocess.run(
            [str(executable), "--help"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        ).stdout.splitlines()[0]

    measurements = {
        "clean_build": clean_build_ms,
        "check_median": round(statistics.median(check_samples_ms), 3),
        "check_p95": round(percentile_95(check_samples_ms), 3),
    }
    result = {
        "schema": 1,
        "platform": platform.platform(),
        "tool": help_output,
        "measurements_ms": measurements,
        "check_samples_ms": check_samples_ms,
        "thresholds_ms": thresholds,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))

    failures = [
        f"{name} took {measurements[name]}ms (limit {limit}ms)"
        for name, limit in thresholds.items()
        if measurements[name] > limit
    ]
    if failures:
        raise RuntimeError("; ".join(failures))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
