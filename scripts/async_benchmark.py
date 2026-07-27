#!/usr/bin/env python3
"""Run versioned async benchmark, zero-cost, and runtime-counter gates."""

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


def summarize_samples(
    samples: list[dict[str, int | None]],
    operations_per_run: int = 1,
    result_schema: int = 1,
) -> dict[str, Any]:
    walls = [int(sample["wall_ns"]) for sample in samples]
    user_cpu = [int(sample["user_cpu_ns"]) for sample in samples]
    system_cpu = [int(sample["system_cpu_ns"]) for sample in samples]
    rss_values = [
        int(sample["peak_rss_bytes"])
        for sample in samples
        if sample["peak_rss_bytes"] is not None
    ]
    fd_values = [
        int(sample["peak_fd_count"])
        for sample in samples
        if sample.get("peak_fd_count") is not None
    ]
    thread_values = [
        int(sample["peak_thread_count"])
        for sample in samples
        if sample.get("peak_thread_count") is not None
    ]
    median = int(statistics.median(walls))
    summary = {
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
    if result_schema >= 2:
        summary.update(
            {
                "operations_per_run": operations_per_run,
                "operations_per_second": round(
                    operations_per_run * 1_000_000_000 / median,
                    3,
                )
                if median
                else 0.0,
                "user_cpu_median_ns": int(statistics.median(user_cpu)),
                "system_cpu_median_ns": int(statistics.median(system_cpu)),
                "peak_fd_count": max(fd_values) if fd_values else None,
                "peak_thread_count": max(thread_values) if thread_values else None,
            }
        )
    return summary


def validate_result(result: dict[str, Any], minimum_runs: int) -> None:
    schema = result.get("schema")
    if schema not in (1, 2) or result.get("suite") != "nomo-async-runtime":
        raise HarnessError("result identity does not match a supported schema")
    claims = result.get("claims", {})
    if claims.get("performance_claim_allowed") is not False:
        raise HarnessError("async benchmark result must not allow a performance claim")
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
            if schema == 2:
                for sample in samples:
                    for field in ("peak_fd_count", "peak_thread_count"):
                        if field not in sample:
                            raise HarnessError(
                                f"{workload.get('id')} {name} is missing {field}"
                            )
                summary = implementation.get("summary", {})
                for field in (
                    "operations_per_run",
                    "operations_per_second",
                    "user_cpu_median_ns",
                    "system_cpu_median_ns",
                    "peak_fd_count",
                    "peak_thread_count",
                ):
                    if field not in summary:
                        raise HarnessError(
                            f"{workload.get('id')} {name} summary is missing {field}"
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
    phase_order = {f"P{index}": index for index in range(7)}
    phase = manifest.get("phase")
    if phase not in phase_order:
        raise HarnessError(f"unsupported async benchmark phase: {phase}")
    result_schema_version = int(manifest.get("result_schema_version", 1))
    if result_schema_version not in (1, 2):
        raise HarnessError("unsupported async benchmark result schema")
    if result_schema_version == 2 and phase_order[phase] < phase_order["P2"]:
        raise HarnessError("result schema 2 requires a P2 or later manifest")
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
    for workload in workloads:
        available_phase = workload.get("available_phase")
        if available_phase not in phase_order:
            raise HarnessError(
                f"workload {workload.get('id')} has an invalid available phase"
            )
        if workload.get("enabled") and phase_order[available_phase] > phase_order[phase]:
            raise HarnessError(
                f"workload {workload.get('id')} cannot be enabled before {available_phase}"
            )
        expected_exit_code = workload.get("expected_exit_code", 0)
        if type(expected_exit_code) is not int or not 0 <= expected_exit_code <= 255:
            raise HarnessError(
                f"workload {workload.get('id')} has an invalid expected exit code"
            )
        if not isinstance(workload.get("expected_stdout", ""), str) or not isinstance(
            workload.get("expected_stderr", ""), str
        ):
            raise HarnessError(
                f"workload {workload.get('id')} expected output must be text"
            )
        operations_per_run = workload.get("operations_per_run", 1)
        if type(operations_per_run) is not int or operations_per_run < 1:
            raise HarnessError(
                f"workload {workload.get('id')} has invalid operations_per_run"
            )
        resource_limits = workload.get("resource_limits", {})
        for field in (
            "requested_cpu_cores",
            "address_space_limit_bytes",
            "peak_rss_budget_bytes",
        ):
            value = resource_limits.get(field)
            if value is not None and (type(value) is not int or value < 1):
                raise HarnessError(
                    f"workload {workload.get('id')} has invalid {field}"
                )
        fixture = workload.get("fixture")
        if fixture is not None and (
            not isinstance(fixture.get("source"), str)
            or not isinstance(fixture.get("environment"), str)
        ):
            raise HarnessError(
                f"workload {workload.get('id')} has an invalid fixture"
            )
    if phase == "P1" and not any(
        workload.get("enabled") and workload.get("kind") == "runtime_counter_gate"
        for workload in workloads
    ):
        raise HarnessError("P1 manifest must enable a runtime counter gate")


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
        if counter.get("available_phase") == "P1" and not counter.get("semantics"):
            raise HarnessError(
                f"P1 counter {counter.get('name')} must define exact semantics"
            )


def validate_runtime_counter_payload(
    payload: dict[str, Any],
    catalog: dict[str, Any],
    expected: dict[str, Any],
) -> None:
    required_top_level = {
        "schema",
        "runtime",
        "runtime_abi",
        "counter_catalog_schema",
        "counters",
        "unavailable",
    }
    if set(payload) != required_top_level:
        raise HarnessError("runtime counter payload has unexpected top-level fields")
    if payload.get("schema") != 1:
        raise HarnessError("unsupported runtime counter payload schema")
    if payload.get("runtime") != "nomo-c99-current-thread":
        raise HarnessError("runtime counter payload has an unexpected runtime")
    if payload.get("runtime_abi") != 1:
        raise HarnessError("runtime counter payload has an unexpected ABI")
    if payload.get("counter_catalog_schema") != catalog.get("schema"):
        raise HarnessError("runtime counter payload catalog schema does not match")

    counters = payload.get("counters")
    unavailable = payload.get("unavailable")
    if not isinstance(counters, dict) or not isinstance(unavailable, dict):
        raise HarnessError("runtime counters and unavailable entries must be objects")
    catalog_entries = {
        str(counter["name"]): counter for counter in catalog.get("counters", [])
    }
    unknown = sorted((set(counters) | set(unavailable)).difference(catalog_entries))
    if unknown:
        raise HarnessError(
            f"runtime counter payload contains unknown counters: {', '.join(unknown)}"
        )
    overlap = sorted(set(counters).intersection(unavailable))
    if overlap:
        raise HarnessError(
            f"runtime counters cannot be both available and unavailable: {', '.join(overlap)}"
        )
    for name, value in counters.items():
        if type(value) is not int or value < 0:
            raise HarnessError(f"runtime counter {name} must be a non-negative integer")
    for name, reason in unavailable.items():
        if not isinstance(reason, str) or not reason:
            raise HarnessError(f"unavailable runtime counter {name} needs a reason")

    p1_names = {
        name
        for name, entry in catalog_entries.items()
        if entry.get("available_phase") == "P1"
    }
    unaccounted = sorted(p1_names.difference(counters, unavailable))
    if unaccounted:
        raise HarnessError(
            f"runtime counter payload does not account for P1 counters: {', '.join(unaccounted)}"
        )

    for name, value in expected.get("counters", {}).items():
        if counters.get(name) != value:
            raise HarnessError(
                f"runtime counter {name} expected {value}, found {counters.get(name)}"
            )
    for name, value in expected.get("counter_minimums", {}).items():
        actual = counters.get(name)
        if actual is None or actual < value:
            raise HarnessError(
                f"runtime counter {name} expected at least {value}, found {actual}"
            )
    for name, value in expected.get("counter_maximums", {}).items():
        actual = counters.get(name)
        if actual is None or actual > value:
            raise HarnessError(
                f"runtime counter {name} expected at most {value}, found {actual}"
            )
    missing_unavailable = sorted(
        set(expected.get("unavailable", [])).difference(unavailable)
    )
    if missing_unavailable:
        raise HarnessError(
            "runtime counter payload is missing unavailable entries: "
            + ", ".join(missing_unavailable)
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
    executable = temporary_root / f"go-{source.name}"
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


def build_fixture(
    c_compiler: Path,
    manifest_root: Path,
    fixture: dict[str, Any],
    temporary_root: Path,
    timeout_seconds: float,
) -> tuple[Path, dict[str, Any]]:
    source = (manifest_root / fixture["source"]).resolve()
    if not source.is_file():
        raise HarnessError(f"fixture source does not exist: {source}")
    executable = temporary_root / f"fixture-{source.parent.name}"
    run_checked(
        [
            str(c_compiler),
            "-std=c99",
            *[str(flag) for flag in fixture.get("build_flags", [])],
            str(source),
            "-o",
            str(executable),
        ],
        timeout_seconds=timeout_seconds,
    )
    return executable, {
        "source": str(source.relative_to(REPOSITORY_ROOT)),
        "source_sha256": sha256_file(source),
        "binary_sha256": sha256_file(executable),
        "environment": fixture["environment"],
    }


def child_setup(resource_limits: dict[str, Any]):
    requested_cpu_cores = resource_limits.get("requested_cpu_cores")
    address_space_limit = resource_limits.get("address_space_limit_bytes")
    limit_platforms = resource_limits.get(
        "address_space_limit_platforms",
        ["Linux"],
    )
    enforce_affinity = (
        requested_cpu_cores == 1
        and platform.system() == "Linux"
        and hasattr(os, "sched_setaffinity")
    )
    enforce_address_space = (
        address_space_limit is not None and platform.system() in limit_platforms
    )
    if not enforce_affinity and not enforce_address_space:
        return None

    def apply() -> None:
        if enforce_affinity:
            available = sorted(os.sched_getaffinity(0))
            if not available:
                raise OSError("no CPU is available for benchmark affinity")
            os.sched_setaffinity(0, {available[0]})
        if enforce_address_space:
            resource.setrlimit(
                resource.RLIMIT_AS,
                (int(address_space_limit), int(address_space_limit)),
            )

    return apply


def resource_control_metadata(resource_limits: dict[str, Any]) -> dict[str, Any]:
    requested_cpu_cores = resource_limits.get("requested_cpu_cores")
    address_space_limit = resource_limits.get("address_space_limit_bytes")
    address_space_platforms = resource_limits.get(
        "address_space_limit_platforms",
        ["Linux"],
    )
    return {
        "requested_cpu_cores": requested_cpu_cores,
        "affinity_enforced": bool(
            requested_cpu_cores == 1
            and platform.system() == "Linux"
            and hasattr(os, "sched_setaffinity")
        ),
        "address_space_limit_bytes": address_space_limit,
        "address_space_limit_enforced": bool(
            address_space_limit is not None
            and platform.system() in address_space_platforms
        ),
        "peak_rss_budget_bytes": resource_limits.get("peak_rss_budget_bytes"),
    }


def linux_process_observation(pid: int) -> tuple[int | None, int | None]:
    if platform.system() != "Linux":
        return None, None
    fd_count = None
    thread_count = None
    try:
        fd_count = len(list(Path(f"/proc/{pid}/fd").iterdir()))
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        pass
    try:
        status = Path(f"/proc/{pid}/status").read_text(encoding="utf-8")
        for line in status.splitlines():
            if line.startswith("Threads:"):
                thread_count = int(line.split(":", 1)[1].strip())
                break
    except (FileNotFoundError, PermissionError, ProcessLookupError, ValueError):
        pass
    return fd_count, thread_count


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
    required_counts = {
        pattern: generated.count(pattern)
        for pattern in snapshot.get("required_generated_c_pattern_counts", {})
    }
    required_failures = {
        pattern: {
            "expected": snapshot["required_generated_c_pattern_counts"][pattern],
            "found": count,
        }
        for pattern, count in required_counts.items()
        if count != snapshot["required_generated_c_pattern_counts"][pattern]
    }
    if required_failures:
        rendered = ", ".join(
            f"{pattern}=expected:{details['expected']},found:{details['found']}"
            for pattern, details in required_failures.items()
        )
        raise HarnessError(f"{snapshot['gate']} generated-C gate failed: {rendered}")
    return {
        "gate": snapshot["gate"],
        "generated_c_sha256": sha256_file(main_c),
        "forbidden_pattern_counts": counts,
        "required_pattern_counts": required_counts,
        "passed": True,
    }


def timed_run(
    command: list[str],
    expected_stdout: bytes,
    expected_stderr: bytes,
    expected_exit_code: int,
    timeout_seconds: float,
    *,
    env: dict[str, str] | None = None,
    resource_limits: dict[str, Any] | None = None,
    result_schema: int = 1,
) -> tuple[dict[str, int | None], bytes]:
    limits = resource_limits or {}
    with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
        started = time.perf_counter_ns()
        process = subprocess.Popen(
            command,
            env=env,
            stdout=stdout_file,
            stderr=stderr_file,
            start_new_session=True,
            preexec_fn=child_setup(limits),
        )
        deadline = time.monotonic() + timeout_seconds
        usage = None
        status = None
        peak_fd_count = None
        peak_thread_count = None
        while time.monotonic() < deadline:
            fd_count, thread_count = linux_process_observation(process.pid)
            if fd_count is not None:
                peak_fd_count = max(peak_fd_count or 0, fd_count)
            if thread_count is not None:
                peak_thread_count = max(peak_thread_count or 0, thread_count)
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
    if process.returncode != expected_exit_code:
        raise HarnessError(
            f"exit status mismatch for {' '.join(command)}: "
            f"expected {expected_exit_code}, found {process.returncode}\n"
            f"{stdout.decode('utf-8', errors='replace')}"
            f"{stderr.decode('utf-8', errors='replace')}"
        )
    if stdout != expected_stdout:
        raise HarnessError(
            f"output mismatch for {' '.join(command)}: "
            f"expected {expected_stdout!r}, found {stdout!r}"
        )
    if stderr != expected_stderr:
        raise HarnessError(
            f"stderr mismatch for {' '.join(command)}: "
            f"expected {expected_stderr!r}, found {stderr!r}"
        )
    rss_scale = 1024 if platform.system() == "Linux" else 1
    sample = {
        "wall_ns": elapsed,
        "user_cpu_ns": max(0, int(usage.ru_utime * 1_000_000_000)),
        "system_cpu_ns": max(0, int(usage.ru_stime * 1_000_000_000)),
        "peak_rss_bytes": max(0, int(usage.ru_maxrss * rss_scale)),
    }
    if result_schema >= 2:
        sample["peak_fd_count"] = peak_fd_count
        sample["peak_thread_count"] = peak_thread_count
    rss_budget = limits.get("peak_rss_budget_bytes")
    if rss_budget is not None and sample["peak_rss_bytes"] > rss_budget:
        raise HarnessError(
            f"peak RSS budget exceeded for {' '.join(command)}: "
            f"limit {rss_budget}, found {sample['peak_rss_bytes']}"
        )
    return sample, stdout


def measure_implementation(
    executable: Path,
    source_sha256: str,
    expected_stdout: bytes,
    expected_stderr: bytes,
    expected_exit_code: int,
    warmup_runs: int,
    measured_runs: int,
    timeout_seconds: float,
    *,
    env: dict[str, str] | None = None,
    resource_limits: dict[str, Any] | None = None,
    operations_per_run: int = 1,
    result_schema: int = 1,
) -> dict[str, Any]:
    limits = resource_limits or {}
    for _ in range(warmup_runs):
        completed = subprocess.run(
            [str(executable)],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            check=False,
            preexec_fn=child_setup(limits),
        )
        if (
            completed.returncode != expected_exit_code
            or completed.stdout != expected_stdout
            or completed.stderr != expected_stderr
        ):
            raise HarnessError(f"warm-up output mismatch for {executable}")
    samples = []
    stdout = b""
    for _ in range(measured_runs):
        sample, stdout = timed_run(
            [str(executable)],
            expected_stdout,
            expected_stderr,
            expected_exit_code,
            timeout_seconds,
            env=env,
            resource_limits=limits,
            result_schema=result_schema,
        )
        samples.append(sample)
    return {
        "source_sha256": source_sha256,
        "binary_sha256": sha256_file(executable),
        "stdout_sha256": sha256_bytes(stdout),
        "samples": samples,
        "summary": summarize_samples(
            samples,
            operations_per_run,
            result_schema,
        ),
    }


def compare_implementations(
    implementations: dict[str, dict[str, Any]],
    *,
    fixture: dict[str, Any] | None,
    resource_limits: dict[str, Any],
    note: str,
) -> dict[str, Any]:
    if "nomo" not in implementations or "go" not in implementations:
        return {
            "performed": False,
            "reason": note,
        }
    nomo = implementations["nomo"]["summary"]
    go = implementations["go"]["summary"]
    nomo_rss = nomo.get("peak_rss_bytes")
    go_rss = go.get("peak_rss_bytes")
    same_output = (
        implementations["nomo"]["stdout_sha256"]
        == implementations["go"]["stdout_sha256"]
    )
    if not same_output:
        raise HarnessError("Nomo and Go benchmark output bytes differ")
    return {
        "performed": True,
        "claim_allowed": False,
        "reason": note,
        "same_output_bytes": same_output,
        "nomo_over_go": {
            "throughput_ratio": round(
                float(nomo["operations_per_second"])
                / float(go["operations_per_second"]),
                6,
            )
            if go["operations_per_second"]
            else None,
            "p99_wall_ratio": round(
                float(nomo["wall_p99_ns"]) / float(go["wall_p99_ns"]),
                6,
            )
            if go["wall_p99_ns"]
            else None,
            "peak_rss_ratio": round(float(nomo_rss) / float(go_rss), 6)
            if nomo_rss is not None and go_rss
            else None,
        },
        "resource_controls": resource_control_metadata(resource_limits),
        "fixture": fixture,
    }


def probe_runtime_counters(
    executable: Path,
    workload_id: str,
    expected_stdout: bytes,
    expected_stderr: bytes,
    expected_exit_code: int,
    runtime_spec: dict[str, Any],
    catalog: dict[str, Any],
    temporary_root: Path,
    timeout_seconds: float,
    *,
    env: dict[str, str] | None = None,
    resource_limits: dict[str, Any] | None = None,
) -> dict[str, Any]:
    metrics_path = temporary_root / f"{workload_id}-runtime-counters.json"
    probe_env = (env or os.environ).copy()
    probe_env["NOMO_ASYNC_METRICS_PATH"] = str(metrics_path)
    completed = subprocess.run(
        [str(executable)],
        env=probe_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
        preexec_fn=child_setup(resource_limits or {}),
    )
    if completed.returncode != expected_exit_code:
        raise HarnessError(
            f"runtime counter probe exit status mismatch for {executable}: "
            f"expected {expected_exit_code}, found {completed.returncode}"
        )
    if completed.stdout != expected_stdout:
        raise HarnessError(f"runtime counter probe output mismatch for {executable}")
    if completed.stderr != expected_stderr:
        raise HarnessError(
            f"runtime counter probe stderr mismatch for {executable}: "
            f"expected {expected_stderr!r}, found {completed.stderr!r}"
        )
    if not metrics_path.is_file():
        raise HarnessError("runtime counter probe did not write its payload")
    try:
        payload = json.loads(metrics_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise HarnessError(f"runtime counter payload is not valid JSON: {error}") from error
    if not isinstance(payload, dict):
        raise HarnessError("runtime counter payload must be an object")
    validate_runtime_counter_payload(payload, catalog, runtime_spec)
    return payload


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
    result_schema = int(manifest.get("result_schema_version", 1))
    result_schema_path = manifest_root / manifest["result_schema"]
    result_schema_document = json.loads(result_schema_path.read_text(encoding="utf-8"))
    declared_schema = (
        result_schema_document.get("properties", {})
        .get("schema", {})
        .get("const")
    )
    if declared_schema != result_schema:
        raise HarnessError(
            f"result schema document declares {declared_schema}, "
            f"manifest requires {result_schema}"
        )
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

            resource_limits = dict(workload.get("resource_limits", {}))
            operations_per_run = int(workload.get("operations_per_run", 1))
            runtime_env = os.environ.copy()
            runtime_env.update(
                {
                    str(name): str(value)
                    for name, value in workload.get("environment", {}).items()
                }
            )
            fixture_metadata = None
            if "fixture" in workload:
                fixture_executable, fixture_metadata = build_fixture(
                    c_compiler,
                    manifest_root,
                    workload["fixture"],
                    temporary_root,
                    timeout_seconds,
                )
                runtime_env[workload["fixture"]["environment"]] = str(
                    fixture_executable
                )
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
            expected_stderr = workload.get("expected_stderr", "").encode("utf-8")
            expected_exit_code = int(workload.get("expected_exit_code", 0))
            implementations = {
                "nomo": measure_implementation(
                    nomo_executable,
                    nomo_source_sha,
                    expected_stdout,
                    expected_stderr,
                    expected_exit_code,
                    warmup_runs,
                    measured_runs,
                    timeout_seconds,
                    env=runtime_env,
                    resource_limits=resource_limits,
                    operations_per_run=operations_per_run,
                    result_schema=result_schema,
                )
            }
            runtime_counters = snapshot["runtime_counters"]
            if runtime_counters.get("available") is True:
                runtime_counters = probe_runtime_counters(
                    nomo_executable,
                    workload["id"],
                    expected_stdout,
                    expected_stderr,
                    expected_exit_code,
                    runtime_counters,
                    counter_catalog,
                    temporary_root,
                    timeout_seconds,
                    env=runtime_env,
                    resource_limits=resource_limits,
                )

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
                go_runtime_env = runtime_env.copy()
                if result_schema >= 2:
                    go_runtime_env.update(
                        {
                            str(name): str(value)
                            for name, value in manifest["toolchains"]["go"][
                                "environment"
                            ].items()
                        }
                    )
                implementations["go"] = measure_implementation(
                    go_executable,
                    go_source_sha,
                    expected_stdout,
                    expected_stderr,
                    expected_exit_code,
                    warmup_runs,
                    measured_runs,
                    timeout_seconds,
                    env=go_runtime_env,
                    resource_limits=resource_limits,
                    operations_per_run=operations_per_run,
                    result_schema=result_schema,
                )

            comparison_note = workload.get(
                "note",
                "This evidence does not authorize a cross-language performance claim",
            )
            if result_schema >= 2:
                comparison = compare_implementations(
                    implementations,
                    fixture=fixture_metadata,
                    resource_limits=resource_limits,
                    note=comparison_note,
                )
            else:
                comparison = {
                    "performed": False,
                    "reason": comparison_note,
                }
            results.append(
                {
                    "id": workload["id"],
                    "kind": workload["kind"],
                    "available_phase": workload["available_phase"],
                    "status": "measured",
                    "claim_eligible": bool(workload["claim_eligible"]),
                    "implementations": implementations,
                    "static_gate": static_gate,
                    "runtime_counters": runtime_counters,
                    "comparison": comparison,
                }
            )

    result = {
        "schema": result_schema,
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
            "result_schema_sha256": sha256_file(result_schema_path),
            "resource_note": (
                f"{manifest['phase']} controls record per-process wall, CPU, and "
                "POSIX wait4 peak RSS. Result schema 2 additionally samples Linux "
                "peak fd/thread counts and enforces declared affinity, address-space, "
                "and peak-RSS controls."
            ),
        },
        "workloads": results,
        "claims": {
            "performance_claim_allowed": False,
            "reason": (
                f"{manifest['phase']} evidence validates only the implemented "
                "generated-C and runtime-counter gates; no production reactor "
                "performance claim is available."
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
