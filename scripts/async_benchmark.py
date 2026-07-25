#!/usr/bin/env python3
"""Run the versioned P0 async benchmark and zero-cost control harness."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import resource
import shutil
import signal
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPOSITORY_ROOT / "performance" / "async" / "manifest.json"


class HarnessError(RuntimeError):
    """An invalid manifest, build, gate, or measurement result."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_tree(path: Path) -> str:
    digest = hashlib.sha256()
    for source in sorted(candidate for candidate in path.rglob("*") if candidate.is_file()):
        relative = source.relative_to(path).as_posix().encode("utf-8")
        contents = source.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(contents).to_bytes(8, "big"))
        digest.update(contents)
    return digest.hexdigest()


def run_checked(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout_seconds: float = 30.0,
) -> subprocess.CompletedProcess[bytes]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
    )
    if completed.returncode != 0:
        stdout = completed.stdout.decode("utf-8", errors="replace")
        stderr = completed.stderr.decode("utf-8", errors="replace")
        raise HarnessError(
            f"command failed ({' '.join(command)}):\n{stdout}{stderr}"
        )
    return completed


def first_line(command: list[str]) -> str:
    output = run_checked(command).stdout.decode("utf-8", errors="replace")
    return output.splitlines()[0] if output.splitlines() else ""


def nearest_rank(values: list[int], percentile: float) -> int:
    if not values:
        raise HarnessError("cannot summarize an empty sample")
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(percentile * len(ordered)) - 1))
    return ordered[index]


def summarize_samples(samples: list[dict[str, int | None]]) -> dict[str, Any]:
    walls = [int(sample["wall_ns"]) for sample in samples]
    rss_values = [
        int(sample["peak_rss_bytes"])
        for sample in samples
        if sample["peak_rss_bytes"] is not None
    ]
    median = int(statistics.median(walls))
    return {
        "runs": len(samples),
        "wall_median_ns": median,
        "wall_p50_ns": nearest_rank(walls, 0.50),
        "wall_p99_ns": nearest_rank(walls, 0.99),
        "wall_p999_ns": nearest_rank(walls, 0.999),
        "executions_per_second": round(1_000_000_000 / median, 3)
        if median
        else 0.0,
        "peak_rss_bytes": max(rss_values) if rss_values else None,
    }


def validate_result(result: dict[str, Any], minimum_runs: int) -> None:
    if result.get("schema") != 1 or result.get("suite") != "nomo-async-runtime":
        raise HarnessError("result identity does not match schema version 1")
    claims = result.get("claims", {})
    if claims.get("performance_claim_allowed") is not False:
        raise HarnessError("P0 result must not allow a performance claim")
    workloads = result.get("workloads", [])
    ids = [workload.get("id") for workload in workloads]
    if len(ids) != len(set(ids)):
        raise HarnessError("result workload ids must be unique")
    for workload in workloads:
        if result.get("phase") == "P0" and workload.get("status") == "measured":
            if workload.get("claim_eligible"):
                raise HarnessError(
                    f"P0 workload {workload.get('id')} cannot be claim eligible"
                )
        for name, implementation in workload.get("implementations", {}).items():
            samples = implementation.get("samples", [])
            if len(samples) < minimum_runs:
                raise HarnessError(
                    f"{workload.get('id')} {name} has fewer than {minimum_runs} samples"
                )
            for field in (
                "source_sha256",
                "binary_sha256",
                "stdout_sha256",
            ):
                digest = implementation.get(field, "")
                if len(digest) != 64 or any(
                    character not in "0123456789abcdef" for character in digest
                ):
                    raise HarnessError(
                        f"{workload.get('id')} {name} has invalid {field}"
                    )


def memory_bytes() -> int | None:
    try:
        page_size = os.sysconf("SC_PAGE_SIZE")
        page_count = os.sysconf("SC_PHYS_PAGES")
    except (AttributeError, OSError, ValueError):
        return None
    return int(page_size * page_count)


def fd_limit(value: int) -> int | str:
    return "infinity" if value == resource.RLIM_INFINITY else int(value)


def host_metadata() -> dict[str, Any]:
    logical = os.cpu_count() or 1
    affinity = getattr(os, "sched_getaffinity", None)
    available = len(affinity(0)) if affinity else logical
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "logical_cpu_count": logical,
        "available_cpu_count": available,
        "memory_bytes": memory_bytes(),
        "power_mode": os.environ.get("NOMO_BENCH_POWER_MODE", "unknown-unreported"),
        "fd_limit_soft": fd_limit(soft),
        "fd_limit_hard": fd_limit(hard),
    }


def resolve_executable(value: Path | str) -> Path:
    raw = str(value)
    found = shutil.which(raw)
    path = Path(found) if found else Path(raw)
    path = path.expanduser().resolve()
    if not path.is_file():
        raise HarnessError(f"executable does not exist: {path}")
    return path


def toolchain_metadata(path: Path, version: str) -> dict[str, str]:
    return {
        "path": str(path),
        "version": version,
        "sha256": sha256_file(path),
    }


def validate_manifest(manifest: dict[str, Any]) -> None:
    if manifest.get("schema") != 1:
        raise HarnessError("unsupported async benchmark manifest schema")
    defaults = manifest.get("defaults", {})
    if int(defaults.get("measured_runs", 0)) < 5:
        raise HarnessError("manifest must require at least five measured runs")
    go_version = manifest.get("toolchains", {}).get("go", {}).get("version", "")
    if not go_version.startswith("go") or go_version.count(".") != 2:
        raise HarnessError("manifest must pin an exact Go patch version")
    workloads = manifest.get("workloads", [])
    ids = [workload.get("id") for workload in workloads]
    if len(ids) != len(set(ids)):
        raise HarnessError("workload ids must be unique")
    required = {
        "task_spawn_complete",
        "idle_suspended_tasks",
        "timer_wheel",
        "bounded_channel",
        "tcp_echo",
        "http_keep_alive",
        "sse_mcp_stream",
        "process_pipe",
        "connection_churn",
        "cancellation_storm",
    }
    missing = sorted(required.difference(ids))
    if missing:
        raise HarnessError(f"manifest is missing required workloads: {', '.join(missing)}")


def validate_counter_catalog(catalog: dict[str, Any]) -> None:
    if catalog.get("schema") != 1:
        raise HarnessError("unsupported async counter catalog schema")
    counters = catalog.get("counters", [])
    names = [counter.get("name") for counter in counters]
    if not counters or len(names) != len(set(names)):
        raise HarnessError("counter catalog names must be non-empty and unique")
    for counter in counters:
        if not counter.get("unit") or not counter.get("available_phase"):
            raise HarnessError(
                f"counter {counter.get('name')} must define unit and available phase"
            )


def find_single(root: Path, pattern: str, label: str) -> Path:
    matches = [path for path in root.rglob(pattern) if path.is_file()]
    if len(matches) != 1:
        rendered = ", ".join(str(path) for path in matches)
        raise HarnessError(f"expected one {label}, found {len(matches)}: {rendered}")
    return matches[0]


def build_nomo_project(
    nomo: Path,
    source: Path,
    temporary_root: Path,
    timeout_seconds: float,
) -> tuple[Path, Path, str]:
    project = temporary_root / f"nomo-{source.name}"
    shutil.copytree(source, project)
    run_checked(
        [str(nomo), "build", str(project)],
        timeout_seconds=timeout_seconds,
    )
    main_c = find_single(project / "build", "main.c", "generated main.c")
    package_name = ""
    for line in (project / "nomo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("name = "):
            package_name = line.split("=", 1)[1].strip().strip('"')
            break
    if not package_name:
        raise HarnessError(f"missing package name in {project / 'nomo.toml'}")
    executable = find_single(project / "build", package_name, "Nomo executable")
    return executable, main_c, sha256_tree(source)


def build_go_project(
    go: Path,
    source: Path,
    temporary_root: Path,
    build_flags: list[str],
    environment: dict[str, str],
    timeout_seconds: float,
) -> tuple[Path, str]:
    executable = temporary_root / "go-ready-call-control"
    env = os.environ.copy()
    env.update(environment)
    env["GOCACHE"] = str(temporary_root / "go-build-cache")
    env["GOMODCACHE"] = str(temporary_root / "go-module-cache")
    env["GOPATH"] = str(temporary_root / "go-path")
    run_checked(
        [str(go), "build", *build_flags, "-o", str(executable), "."],
        cwd=source,
        env=env,
        timeout_seconds=timeout_seconds,
    )
    return executable, sha256_tree(source)


def scan_static_gate(main_c: Path, snapshot: dict[str, Any]) -> dict[str, Any]:
    generated = main_c.read_text(encoding="utf-8")
    counts = {
        pattern: generated.count(pattern)
        for pattern in snapshot["required_absent_generated_c_patterns"]
    }
    failures = {pattern: count for pattern, count in counts.items() if count != 0}
    if failures:
        rendered = ", ".join(f"{pattern}={count}" for pattern, count in failures.items())
        raise HarnessError(f"{snapshot['gate']} generated-C gate failed: {rendered}")
    return {
        "gate": snapshot["gate"],
        "generated_c_sha256": sha256_file(main_c),
        "forbidden_pattern_counts": counts,
        "passed": True,
    }


def timed_run(
    command: list[str],
    expected_stdout: bytes,
    timeout_seconds: float,
) -> tuple[dict[str, int | None], bytes]:
    with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
        started = time.perf_counter_ns()
        process = subprocess.Popen(
            command,
            stdout=stdout_file,
            stderr=stderr_file,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout_seconds
        usage = None
        status = None
        while time.monotonic() < deadline:
            waited_pid, waited_status, waited_usage = os.wait4(process.pid, os.WNOHANG)
            if waited_pid == process.pid:
                status = waited_status
                usage = waited_usage
                break
            time.sleep(0.001)
        if status is None or usage is None:
            waited_pid, waited_status, waited_usage = os.wait4(process.pid, os.WNOHANG)
            if waited_pid == process.pid:
                status = waited_status
                usage = waited_usage
            else:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                os.wait4(process.pid, 0)
        if status is None or usage is None:
            raise HarnessError(
                f"command timed out after {timeout_seconds}s: {' '.join(command)}"
            )
        elapsed = time.perf_counter_ns() - started
        process.returncode = os.waitstatus_to_exitcode(status)
        stdout_file.seek(0)
        stderr_file.seek(0)
        stdout = stdout_file.read()
        stderr = stderr_file.read()
    if process.returncode != 0:
        raise HarnessError(
            f"command failed ({' '.join(command)}):\n"
            f"{stdout.decode('utf-8', errors='replace')}"
            f"{stderr.decode('utf-8', errors='replace')}"
        )
    if stdout != expected_stdout:
        raise HarnessError(
            f"output mismatch for {' '.join(command)}: "
            f"expected {expected_stdout!r}, found {stdout!r}"
        )
    if stderr:
        raise HarnessError(
            f"unexpected stderr for {' '.join(command)}: "
            f"{stderr.decode('utf-8', errors='replace')}"
        )
    rss_scale = 1024 if platform.system() == "Linux" else 1
    return (
        {
            "wall_ns": elapsed,
            "user_cpu_ns": max(0, int(usage.ru_utime * 1_000_000_000)),
            "system_cpu_ns": max(0, int(usage.ru_stime * 1_000_000_000)),
            "peak_rss_bytes": max(0, int(usage.ru_maxrss * rss_scale)),
        },
        stdout,
    )


def measure_implementation(
    executable: Path,
    source_sha256: str,
    expected_stdout: bytes,
    warmup_runs: int,
    measured_runs: int,
    timeout_seconds: float,
) -> dict[str, Any]:
    for _ in range(warmup_runs):
        completed = run_checked([str(executable)], timeout_seconds=timeout_seconds)
        if completed.stdout != expected_stdout or completed.stderr:
            raise HarnessError(f"warm-up output mismatch for {executable}")
    samples = []
    stdout = b""
    for _ in range(measured_runs):
        sample, stdout = timed_run(
            [str(executable)],
            expected_stdout,
            timeout_seconds,
        )
        samples.append(sample)
    return {
        "source_sha256": source_sha256,
        "binary_sha256": sha256_file(executable),
        "stdout_sha256": sha256_bytes(stdout),
        "samples": samples,
        "summary": summarize_samples(samples),
    }


def workload_result_unavailable(workload: dict[str, Any]) -> dict[str, Any]:
    phase = workload["available_phase"]
    return {
        "id": workload["id"],
        "kind": workload["kind"],
        "available_phase": phase,
        "status": "unavailable",
        "claim_eligible": bool(workload["claim_eligible"]),
        "implementations": {},
        "static_gate": None,
        "runtime_counters": {
            "available": False,
            "reason": f"implementation is scheduled for {phase}",
        },
        "comparison": {
            "performed": False,
            "reason": f"implementation is scheduled for {phase}",
        },
    }


def git_revision() -> str:
    return (
        run_checked(["git", "rev-parse", "HEAD"], cwd=REPOSITORY_ROOT)
        .stdout.decode("utf-8")
        .strip()
    )


def repository_status() -> str:
    return (
        run_checked(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            cwd=REPOSITORY_ROOT,
        )
        .stdout.decode("utf-8", errors="replace")
        .strip()
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nomo", type=Path, required=True)
    parser.add_argument("--go", default="go")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--warmup-runs", type=int)
    parser.add_argument("--measured-runs", type=int)
    parser.add_argument(
        "--require-clean",
        action="store_true",
        help="reject evidence produced from a dirty Git checkout",
    )
    args = parser.parse_args()

    manifest_path = args.manifest.resolve()
    manifest_root = manifest_path.parent
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    validate_manifest(manifest)
    counter_catalog_path = manifest_root / manifest["counter_catalog"]
    counter_catalog = json.loads(counter_catalog_path.read_text(encoding="utf-8"))
    validate_counter_catalog(counter_catalog)
    dirty_status = repository_status()
    dirty = bool(dirty_status)
    if args.require_clean and dirty:
        raise HarnessError(
            "benchmark evidence requires a clean Git checkout:\n"
            f"{dirty_status}"
        )
    defaults = manifest["defaults"]
    warmup_runs = (
        args.warmup_runs
        if args.warmup_runs is not None
        else int(defaults["warmup_runs"])
    )
    measured_runs = (
        args.measured_runs
        if args.measured_runs is not None
        else int(defaults["measured_runs"])
    )
    if warmup_runs < 1:
        parser.error("--warmup-runs must be at least 1")
    if measured_runs < int(manifest["comparison_policy"]["minimum_measured_runs"]):
        parser.error("--measured-runs cannot weaken the manifest minimum")
    timeout_seconds = float(defaults["timeout_seconds"])

    nomo = resolve_executable(args.nomo)
    go = resolve_executable(args.go)
    go_version = first_line([str(go), "version"]).split()
    actual_go_version = go_version[2] if len(go_version) >= 3 else ""
    expected_go_version = manifest["toolchains"]["go"]["version"]
    if actual_go_version != expected_go_version:
        raise HarnessError(
            f"Go version mismatch: expected {expected_go_version}, found {actual_go_version}"
        )
    c_compiler = resolve_executable(os.environ.get("CC", "cc"))
    c_version = first_line([str(c_compiler), "--version"])
    nomo_version = first_line([str(nomo), "--help"])

    results = []
    with tempfile.TemporaryDirectory(prefix="nomo-async-benchmark-") as temporary:
        temporary_root = Path(temporary)
        for workload in manifest["workloads"]:
            if not workload["enabled"]:
                results.append(workload_result_unavailable(workload))
                continue

            nomo_source = manifest_root / workload["nomo_project"]
            nomo_executable, main_c, nomo_source_sha = build_nomo_project(
                nomo,
                nomo_source,
                temporary_root,
                timeout_seconds,
            )
            snapshot = json.loads(
                (manifest_root / workload["snapshot"]).read_text(encoding="utf-8")
            )
            static_gate = scan_static_gate(main_c, snapshot)
            expected_stdout = workload["expected_stdout"].encode("utf-8")
            implementations = {
                "nomo": measure_implementation(
                    nomo_executable,
                    nomo_source_sha,
                    expected_stdout,
                    warmup_runs,
                    measured_runs,
                    timeout_seconds,
                )
            }

            if "go_project" in workload:
                go_source = manifest_root / workload["go_project"]
                go_executable, go_source_sha = build_go_project(
                    go,
                    go_source,
                    temporary_root,
                    list(manifest["toolchains"]["go"]["build_flags"]),
                    dict(manifest["toolchains"]["go"]["environment"]),
                    timeout_seconds,
                )
                implementations["go"] = measure_implementation(
                    go_executable,
                    go_source_sha,
                    expected_stdout,
                    warmup_runs,
                    measured_runs,
                    timeout_seconds,
                )

            results.append(
                {
                    "id": workload["id"],
                    "kind": workload["kind"],
                    "available_phase": workload["available_phase"],
                    "status": "measured",
                    "claim_eligible": bool(workload["claim_eligible"]),
                    "implementations": implementations,
                    "static_gate": static_gate,
                    "runtime_counters": snapshot["runtime_counters"],
                    "comparison": {
                        "performed": False,
                        "reason": workload.get(
                            "note",
                            "P0 zero-cost gate has no cross-language performance claim",
                        ),
                    },
                }
            )

    result = {
        "schema": 1,
        "suite": manifest["suite"],
        "phase": manifest["phase"],
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "manifest_sha256": sha256_file(manifest_path),
        "harness_sha256": sha256_file(Path(__file__).resolve()),
        "counter_catalog_sha256": sha256_file(counter_catalog_path),
        "revision": git_revision(),
        "repository_dirty": dirty,
        "host": host_metadata(),
        "toolchains": {
            "nomo": toolchain_metadata(nomo, nomo_version),
            "go": toolchain_metadata(go, first_line([str(go), "version"])),
            "c": toolchain_metadata(c_compiler, c_version),
        },
        "configuration": {
            **defaults,
            "warmup_runs": warmup_runs,
            "measured_runs": measured_runs,
            "resource_note": (
                "P0 control records per-process wall, CPU, and POSIX wait4 peak RSS. "
                "Steady RSS and non-POSIX collectors arrive with async workloads."
            ),
        },
        "workloads": results,
        "claims": {
            "performance_claim_allowed": False,
            "reason": (
                "P0 only validates harness plumbing and zero-cost generated-C gates; "
                "no nonblocking I/O workload or production async runtime exists yet."
            ),
        },
    }
    validate_result(
        result,
        int(manifest["comparison_policy"]["minimum_measured_runs"]),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": str(args.output),
                "revision": result["revision"],
                "repository_dirty": result["repository_dirty"],
                "workloads": [
                    {
                        "id": workload["id"],
                        "status": workload["status"],
                        "claim_eligible": workload["claim_eligible"],
                        "static_gate_passed": (
                            workload["static_gate"]["passed"]
                            if workload["static_gate"] is not None
                            else None
                        ),
                    }
                    for workload in result["workloads"]
                ],
                "claims": result["claims"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
