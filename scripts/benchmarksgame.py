#!/usr/bin/env python3
"""Reproducible single-thread CPU baseline for three Benchmarks Game programs."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import random
import shlex
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Dict, Optional, Sequence, Tuple


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    REPOSITORY_ROOT / "performance" / "benchmarksgame" / "manifest.json"
)
DEFAULT_RESULTS_ROOT = (
    REPOSITORY_ROOT / "performance" / "results" / "benchmarksgame"
)
IMPLEMENTATIONS = ("nomo", "c", "go")
WORKLOAD_IDS = ("spectral-norm", "n-body", "fannkuch-redux")
READINESS_IDS = (
    "spectral-norm",
    "n-body",
    "fannkuch-redux",
    "fasta",
    "mandelbrot",
    "reverse-complement",
    "k-nucleotide",
    "pidigits",
    "regex-redux",
    "binary-trees",
)
BASE_CLANG_FLAGS = (
    "-std=c99",
    "-O3",
    "-DNDEBUG",
    "-fomit-frame-pointer",
)
T_CRITICAL_95 = {
    1: 12.7062047364,
    2: 4.30265272975,
    3: 3.18244630528,
    4: 2.7764451052,
    5: 2.57058183564,
    6: 2.44691184879,
    7: 2.36462425101,
    8: 2.3060041352,
    9: 2.26215716285,
    10: 2.22813885196,
    11: 2.20098516008,
    12: 2.17881282966,
    13: 2.16036865646,
    14: 2.14478668792,
    15: 2.13144954556,
    16: 2.11990529922,
    17: 2.10981557783,
    18: 2.10092204024,
    19: 2.09302405441,
    20: 2.08596344727,
    21: 2.07961384473,
    22: 2.0738730679,
    23: 2.06865761042,
    24: 2.06389856163,
    25: 2.05953855275,
    26: 2.05552943864,
    27: 2.05183051648,
    28: 2.0484071418,
    29: 2.04522964213,
    30: 2.0422724563,
}


class HarnessError(RuntimeError):
    """Raised when the suite contract or a benchmark command fails."""


class ToolchainMismatch(HarnessError):
    """Raised when an installed toolchain does not match the manifest."""


class CommandTimedOut(HarnessError):
    """Raised after a child process is forcefully terminated at the hard timeout."""


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_nomo_project(root: Path) -> str:
    digest = hashlib.sha256()
    paths = [
        path
        for path in root.rglob("*")
        if path.is_file()
        and "build" not in path.relative_to(root).parts
        and ".nomo" not in path.relative_to(root).parts
    ]
    for path in sorted(paths):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        content = path.read_bytes()
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.hexdigest()


def command_text(command: Sequence[str]) -> str:
    return shlex.join([str(part) for part in command])


def read_json(path: Path) -> Dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise HarnessError(f"cannot read JSON from {path}: {error}") from error
    if not isinstance(value, dict):
        raise HarnessError(f"{path} must contain a JSON object")
    return value


def resolve_suite_path(suite_root: Path, relative: str, label: str) -> Path:
    candidate = (suite_root / relative).resolve()
    try:
        candidate.relative_to(suite_root.resolve())
    except ValueError as error:
        raise HarnessError(f"{label} escapes the suite root: {relative}") from error
    if not candidate.is_file():
        raise HarnessError(f"{label} is missing: {candidate}")
    return candidate


def validate_source(
    suite_root: Path,
    source: Dict[str, Any],
    label: str,
    require_upstream: bool,
) -> None:
    path = resolve_suite_path(suite_root, str(source.get("path", "")), label)
    expected_sha = source.get("sha256")
    actual_sha = sha256_file(path)
    if expected_sha != actual_sha:
        raise HarnessError(
            f"{label} SHA-256 mismatch: expected {expected_sha}, found {actual_sha}"
        )
    if require_upstream:
        url = source.get("upstream_url")
        fetched_on = source.get("fetched_on")
        upstream_sha = source.get("upstream_extracted_sha256")
        if (
            not isinstance(url, str)
            or not url.startswith(
                "https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/"
            )
            or not url.endswith("-8.html")
        ):
            raise HarnessError(f"{label} must pin an official #8 program URL")
        if fetched_on != "2026-07-27":
            raise HarnessError(f"{label} must pin the audited fetch date")
        if not isinstance(upstream_sha, str) or len(upstream_sha) != 64:
            raise HarnessError(f"{label} must record the extracted upstream SHA-256")


def validate_manifest(manifest: Dict[str, Any], suite_root: Path) -> None:
    if manifest.get("schema") != 1:
        raise HarnessError("unsupported Benchmarks Game manifest schema")
    if manifest.get("suite") != "nomo-benchmarksgame-cpu-baseline":
        raise HarnessError("unexpected Benchmarks Game suite identity")
    if manifest.get("manifest_version") != "2026-07-27":
        raise HarnessError("unexpected Benchmarks Game manifest version")
    if manifest.get("license") != "BSD-3-Clause":
        raise HarnessError("suite must preserve the BSD-3-Clause license")

    methodology = manifest.get("methodology")
    if not isinstance(methodology, dict):
        raise HarnessError("manifest methodology must be an object")
    required_methodology = {
        "measured_runs": 12,
        "cpu_statistics_exclude_first_run": True,
        "slow_cutoff_seconds": 600,
        "hard_timeout_seconds": 3600,
        "warmup_runs": 0,
        "random_seed": 20260727,
        "affinity_enforced": False,
    }
    for key, expected in required_methodology.items():
        if methodology.get(key) != expected:
            raise HarnessError(f"manifest methodology {key} must be {expected!r}")
    if methodology.get("implementations") != list(IMPLEMENTATIONS):
        raise HarnessError("manifest implementation order must be Nomo, C, Go")
    if methodology.get("stdout_after_first_run") != "/dev/null":
        raise HarnessError("manifest must redirect later stdout to /dev/null")

    toolchains = manifest.get("toolchains")
    if not isinstance(toolchains, dict):
        raise HarnessError("manifest toolchains must be an object")
    if toolchains.get("go", {}).get("required_version") != "go1.25.12":
        raise HarnessError("manifest must pin the Go patch version")
    if tuple(toolchains.get("clang", {}).get("flags", [])) != BASE_CLANG_FLAGS:
        raise HarnessError("manifest must use the fixed Clang comparison flags")
    required_nomo_version = toolchains.get("nomo", {}).get("required_version")
    if not isinstance(required_nomo_version, str) or not required_nomo_version:
        raise HarnessError("manifest must pin the Nomo version")

    workloads = manifest.get("workloads")
    if not isinstance(workloads, list):
        raise HarnessError("manifest workloads must be a list")
    ids = [workload.get("id") for workload in workloads]
    if tuple(ids) != WORKLOAD_IDS:
        raise HarnessError("manifest must define exactly the three CPU workloads")
    for workload in workloads:
        workload_id = str(workload["id"])
        for input_key in ("correctness_input", "performance_input"):
            value = workload.get(input_key)
            if not isinstance(value, str) or not value.isdigit() or int(value) <= 0:
                raise HarnessError(f"{workload_id} has invalid {input_key}")
        fixtures = workload.get("fixtures")
        if not isinstance(fixtures, dict):
            raise HarnessError(f"{workload_id} fixtures must be an object")
        for fixture_kind in ("correctness", "performance"):
            fixture = fixtures.get(fixture_kind)
            if not isinstance(fixture, dict):
                raise HarnessError(
                    f"{workload_id} is missing the {fixture_kind} fixture"
                )
            path = resolve_suite_path(
                suite_root,
                str(fixture.get("path", "")),
                f"{workload_id} {fixture_kind} fixture",
            )
            if fixture.get("sha256") != sha256_file(path):
                raise HarnessError(
                    f"{workload_id} {fixture_kind} fixture SHA-256 mismatch"
                )
        sources = workload.get("sources")
        if not isinstance(sources, dict) or set(sources) != set(IMPLEMENTATIONS):
            raise HarnessError(f"{workload_id} must define Nomo, C, and Go sources")
        validate_source(
            suite_root,
            sources["c"],
            f"{workload_id} C source",
            require_upstream=True,
        )
        validate_source(
            suite_root,
            sources["go"],
            f"{workload_id} Go source",
            require_upstream=True,
        )
        nomo_source = sources["nomo"]
        validate_source(
            suite_root,
            nomo_source,
            f"{workload_id} Nomo source",
            require_upstream=False,
        )
        project = resolve_suite_path(
            suite_root,
            str(nomo_source.get("project_manifest", "")),
            f"{workload_id} Nomo project manifest",
        )
        if project.name != "nomo.toml":
            raise HarnessError(f"{workload_id} Nomo project must use nomo.toml")
        if nomo_source.get("project_manifest_sha256") != sha256_file(project):
            raise HarnessError(
                f"{workload_id} Nomo project manifest SHA-256 mismatch"
            )
        nomo_text = resolve_suite_path(
            suite_root,
            str(nomo_source["path"]),
            f"{workload_id} Nomo source",
        ).read_text(encoding="utf-8")
        forbidden = ("import std.ffi", "suspend fn", "extern ", "unsafe ", "std.task")
        present = [needle for needle in forbidden if needle in nomo_text]
        if present:
            raise HarnessError(
                f"{workload_id} Nomo source uses forbidden facilities: "
                + ", ".join(present)
            )

    readiness = manifest.get("readiness")
    if not isinstance(readiness, list) or len(readiness) != 10:
        raise HarnessError("manifest readiness matrix must contain ten workloads")
    readiness_ids = tuple(item.get("id") for item in readiness)
    if readiness_ids != READINESS_IDS:
        raise HarnessError("manifest readiness matrix is incomplete or reordered")
    expected_statuses = {
        "spectral-norm": "implemented",
        "n-body": "implemented",
        "fannkuch-redux": "implemented",
        "fasta": "deferred-buffered-bytes-io",
        "mandelbrot": "deferred-buffered-bytes-io",
        "reverse-complement": "deferred-buffered-bytes-io",
        "k-nucleotide": "deferred-core-stdlib",
        "pidigits": "deferred-core-stdlib",
        "regex-redux": "deferred-core-stdlib",
        "binary-trees": "deferred-allocation-model",
    }
    for item in readiness:
        if item.get("status") != expected_statuses[item["id"]]:
            raise HarnessError(f"readiness status is inaccurate for {item['id']}")
        if not item.get("reason"):
            raise HarnessError(f"readiness item {item['id']} needs a reason")


def run_capture(
    command: Sequence[str],
    timeout_seconds: float,
    cwd: Optional[Path] = None,
    environment: Optional[Dict[str, str]] = None,
) -> Tuple[Dict[str, Any], bytes, bytes]:
    started = time.perf_counter_ns()
    try:
        completed = subprocess.run(
            [str(part) for part in command],
            cwd=str(cwd) if cwd is not None else None,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise CommandTimedOut(
            f"command exceeded {timeout_seconds:.3f}s: {command_text(command)}"
        ) from error
    duration = time.perf_counter_ns() - started
    record = {
        "argv": [str(part) for part in command],
        "command": command_text(command),
        "cwd": str(cwd.resolve()) if cwd is not None else None,
        "duration_ns": duration,
        "exit_code": completed.returncode,
    }
    if completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace")
        stdout = completed.stdout.decode("utf-8", errors="replace")
        raise HarnessError(
            f"command failed with exit {completed.returncode}: {record['command']}\n"
            f"stdout:\n{stdout}\nstderr:\n{stderr}"
        )
    return record, completed.stdout, completed.stderr


def tool_version(
    executable: Path,
    arguments: Sequence[str],
    timeout_seconds: float = 30.0,
) -> str:
    _, stdout, stderr = run_capture(
        [str(executable), *arguments],
        timeout_seconds=timeout_seconds,
    )
    combined = (stdout + stderr).decode("utf-8", errors="replace").strip()
    if not combined:
        raise HarnessError(f"empty version output from {executable}")
    return combined


def parse_nomo_version(help_output: str) -> str:
    first_line = help_output.splitlines()[0].strip() if help_output else ""
    fields = first_line.split()
    if len(fields) != 2 or fields[0] != "nomo":
        raise ToolchainMismatch(f"unexpected Nomo help banner: {first_line!r}")
    return fields[1]


def inspect_toolchains(
    manifest: Dict[str, Any],
    nomo_argument: Path,
    clang_argument: str,
    go_argument: str,
) -> Dict[str, Any]:
    programs = {}
    for name, value in (
        ("nomo", str(nomo_argument)),
        ("clang", clang_argument),
        ("go", go_argument),
    ):
        resolved = shutil.which(value)
        if resolved is None:
            candidate = Path(value).expanduser()
            if candidate.is_file():
                resolved = str(candidate.resolve())
        if resolved is None:
            raise HarnessError(f"required {name} executable was not found: {value}")
        programs[name] = Path(resolved).resolve()

    nomo_help = tool_version(programs["nomo"], ["--help"])
    nomo_version = parse_nomo_version(nomo_help)
    clang_version = tool_version(programs["clang"], ["--version"])
    go_output = tool_version(programs["go"], ["version"])
    go_fields = go_output.split()
    go_version = go_fields[2] if len(go_fields) >= 3 else ""

    expected_nomo = manifest["toolchains"]["nomo"]["required_version"]
    expected_go = manifest["toolchains"]["go"]["required_version"]
    mismatches = []
    if nomo_version != expected_nomo:
        mismatches.append(f"Nomo expected {expected_nomo}, found {nomo_version}")
    if go_version != expected_go:
        mismatches.append(f"Go expected {expected_go}, found {go_version}")
    if "clang version" not in clang_version.lower():
        mismatches.append("the configured C compiler is not Clang")
    if mismatches:
        raise ToolchainMismatch("toolchain mismatch: " + "; ".join(mismatches))

    return {
        "nomo": {
            "path": str(programs["nomo"]),
            "version": nomo_version,
            "version_output": nomo_help.splitlines()[0],
            "sha256": sha256_file(programs["nomo"]),
        },
        "clang": {
            "path": str(programs["clang"]),
            "version_output": clang_version,
        },
        "go": {
            "path": str(programs["go"]),
            "version": go_version,
            "version_output": go_output,
        },
    }


def git_capture(repository_root: Path, arguments: Sequence[str]) -> str:
    _, stdout, _ = run_capture(
        ["git", *arguments],
        timeout_seconds=30.0,
        cwd=repository_root,
    )
    return stdout.decode("utf-8").strip()


def repository_state(
    repository_root: Path, require_clean: bool
) -> Dict[str, Any]:
    commit = git_capture(repository_root, ["rev-parse", "HEAD"])
    status_text = git_capture(
        repository_root,
        ["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    dirty_paths = status_text.splitlines() if status_text else []
    dirty = bool(dirty_paths)
    if require_clean and dirty:
        raise HarnessError(
            "dirty checkout is not allowed: " + "; ".join(dirty_paths[:20])
        )
    return {
        "commit": commit,
        "dirty": dirty,
        "dirty_paths": dirty_paths,
    }


def copy_nomo_project(source_file: Path, project_manifest: Path, destination: Path) -> None:
    project_root = project_manifest.parent
    shutil.copytree(
        project_root,
        destination,
        ignore=shutil.ignore_patterns("build", ".nomo"),
    )
    copied_source = destination / source_file.relative_to(project_root)
    if not copied_source.is_file():
        raise HarnessError(f"copied Nomo project lost its source: {copied_source}")


def build_workload(
    workload: Dict[str, Any],
    suite_root: Path,
    bundle_root: Path,
    toolchains: Dict[str, Any],
    build_timeout_seconds: float,
) -> Tuple[Dict[str, Any], Dict[str, Path]]:
    workload_id = workload["id"]
    build_root = bundle_root / "build" / workload_id
    binary_root = bundle_root / "bin"
    build_root.mkdir(parents=True, exist_ok=True)
    binary_root.mkdir(parents=True, exist_ok=True)
    sources = workload["sources"]

    nomo_source = resolve_suite_path(
        suite_root, sources["nomo"]["path"], f"{workload_id} Nomo source"
    )
    nomo_manifest = resolve_suite_path(
        suite_root,
        sources["nomo"]["project_manifest"],
        f"{workload_id} Nomo project",
    )
    nomo_project = build_root / "nomo-project"
    copy_nomo_project(nomo_source, nomo_manifest, nomo_project)
    nomo_command = [
        toolchains["nomo"]["path"],
        "build",
        str(nomo_project),
        "--emit-c",
    ]
    nomo_emit_record, nomo_stdout, nomo_stderr = run_capture(
        nomo_command,
        timeout_seconds=build_timeout_seconds,
        cwd=REPOSITORY_ROOT,
    )
    generated_c = nomo_project / "build" / "c" / "main.c"
    if not generated_c.is_file():
        raise HarnessError(f"Nomo did not emit generated C for {workload_id}")

    c_source = resolve_suite_path(
        suite_root, sources["c"]["path"], f"{workload_id} C source"
    )
    go_source = resolve_suite_path(
        suite_root, sources["go"]["path"], f"{workload_id} Go source"
    )
    copied_c = build_root / "reference.c"
    copied_go = build_root / "reference.go"
    shutil.copy2(c_source, copied_c)
    shutil.copy2(go_source, copied_go)

    math_flags = ["-lm"] if workload.get("link_math") else []
    nomo_binary = binary_root / f"{workload_id}-nomo"
    c_binary = binary_root / f"{workload_id}-c"
    go_binary = binary_root / f"{workload_id}-go"
    nomo_clang_command = [
        toolchains["clang"]["path"],
        *BASE_CLANG_FLAGS,
        str(generated_c),
        "-o",
        str(nomo_binary),
        *math_flags,
    ]
    c_clang_command = [
        toolchains["clang"]["path"],
        *BASE_CLANG_FLAGS,
        str(copied_c),
        "-o",
        str(c_binary),
        *math_flags,
    ]
    go_command = [
        toolchains["go"]["path"],
        "build",
        "-o",
        str(go_binary),
        str(copied_go),
    ]
    nomo_clang_record, _, _ = run_capture(
        nomo_clang_command,
        timeout_seconds=build_timeout_seconds,
        cwd=REPOSITORY_ROOT,
    )
    c_clang_record, _, _ = run_capture(
        c_clang_command,
        timeout_seconds=build_timeout_seconds,
        cwd=REPOSITORY_ROOT,
    )
    go_record, _, _ = run_capture(
        go_command,
        timeout_seconds=build_timeout_seconds,
        cwd=REPOSITORY_ROOT,
    )
    for binary in (nomo_binary, c_binary, go_binary):
        if not binary.is_file():
            raise HarnessError(f"build did not produce {binary}")

    record = {
        "source_files": {
            "nomo": [
                {
                    "path": str(nomo_source),
                    "sha256": sha256_file(nomo_source),
                },
                {
                    "path": str(nomo_manifest),
                    "sha256": sha256_file(nomo_manifest),
                },
            ],
            "c": [{"path": str(c_source), "sha256": sha256_file(c_source)}],
            "go": [{"path": str(go_source), "sha256": sha256_file(go_source)}],
        },
        "source_tree_sha256": {
            "nomo": sha256_nomo_project(nomo_manifest.parent),
            "c": sha256_file(c_source),
            "go": sha256_file(go_source),
        },
        "generated_c": {
            "path": str(generated_c.resolve()),
            "sha256": sha256_file(generated_c),
            "unmodified_after_emit": True,
        },
        "binaries": {
            "nomo": {
                "path": str(nomo_binary.resolve()),
                "sha256": sha256_file(nomo_binary),
            },
            "c": {
                "path": str(c_binary.resolve()),
                "sha256": sha256_file(c_binary),
            },
            "go": {
                "path": str(go_binary.resolve()),
                "sha256": sha256_file(go_binary),
            },
        },
        "commands": {
            "nomo_emit_c": nomo_emit_record,
            "nomo_clang": nomo_clang_record,
            "c_clang": c_clang_record,
            "go_build": go_record,
        },
        "nomo_emit_stdout": nomo_stdout.decode("utf-8", errors="replace"),
        "nomo_emit_stderr": nomo_stderr.decode("utf-8", errors="replace"),
        "compile_time_excluded_from_run_time": True,
    }
    binaries = {
        "nomo": nomo_binary.resolve(),
        "c": c_binary.resolve(),
        "go": go_binary.resolve(),
    }
    return record, binaries


def peak_rss_bytes(value: int) -> int:
    return int(value) if sys.platform == "darwin" else int(value) * 1024


def timed_run(
    command: Sequence[str],
    expected_stdout: Optional[bytes],
    timeout_seconds: float,
    environment_overrides: Optional[Dict[str, str]] = None,
) -> Tuple[Dict[str, Any], Optional[bytes]]:
    environment = os.environ.copy()
    environment["LC_ALL"] = "C"
    environment["LANG"] = "C"
    overrides = dict(environment_overrides or {})
    environment.update(overrides)
    capture_stdout = expected_stdout is not None
    with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
        stdout_target = stdout_file if capture_stdout else subprocess.DEVNULL
        process = subprocess.Popen(
            [str(part) for part in command],
            stdin=subprocess.DEVNULL,
            stdout=stdout_target,
            stderr=stderr_file,
            env=environment,
            start_new_session=True,
        )
        started = time.perf_counter_ns()
        usage = None
        status = None
        deadline = time.monotonic() + timeout_seconds
        while usage is None:
            waited_pid, waited_status, waited_usage = os.wait4(
                process.pid, os.WNOHANG
            )
            if waited_pid == process.pid:
                usage = waited_usage
                status = waited_status
                break
            if time.monotonic() >= deadline:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                _, waited_status, waited_usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(waited_status)
                raise CommandTimedOut(
                    f"command exceeded {timeout_seconds:.3f}s and was killed: "
                    f"{command_text(command)}"
                )
            time.sleep(0.005)
        wall_ns = time.perf_counter_ns() - started
        assert usage is not None
        assert status is not None
        exit_code = os.waitstatus_to_exitcode(status)
        process.returncode = exit_code
        stderr_file.seek(0)
        stderr = stderr_file.read()
        stdout = None
        if capture_stdout:
            stdout_file.seek(0)
            stdout = stdout_file.read()
        if exit_code != 0:
            raise HarnessError(
                f"benchmark exited {exit_code}: {command_text(command)}\n"
                f"stderr:\n{stderr.decode('utf-8', errors='replace')}"
            )
        if stderr:
            raise HarnessError(
                f"benchmark wrote unexpected stderr: {command_text(command)}\n"
                + stderr.decode("utf-8", errors="replace")
            )
        if expected_stdout is not None and stdout != expected_stdout:
            raise HarnessError(
                f"output mismatch for {command_text(command)}: "
                f"expected {expected_stdout!r}, found {stdout!r}"
            )
        sample = {
            "command_argv": [str(part) for part in command],
            "command": command_text(command),
            "environment": {
                "LC_ALL": "C",
                "LANG": "C",
                **overrides,
            },
            "stdout": "captured-and-verified" if capture_stdout else "/dev/null",
            "wall_ns": wall_ns,
            "user_cpu_ns": int(round(usage.ru_utime * 1_000_000_000)),
            "system_cpu_ns": int(round(usage.ru_stime * 1_000_000_000)),
            "cpu_total_ns": int(
                round((usage.ru_utime + usage.ru_stime) * 1_000_000_000)
            ),
            "peak_rss_bytes": peak_rss_bytes(usage.ru_maxrss),
            "exit_code": exit_code,
        }
        return sample, stdout


def confidence_interval_95(values: Sequence[int]) -> Dict[str, Any]:
    if len(values) < 2:
        raise HarnessError("95% t-confidence interval needs at least two samples")
    sample_count = len(values)
    degrees_of_freedom = sample_count - 1
    if degrees_of_freedom not in T_CRITICAL_95:
        raise HarnessError("95% t-confidence interval table supports up to 31 samples")
    mean = statistics.fmean(values)
    standard_deviation = statistics.stdev(values)
    standard_error = standard_deviation / math.sqrt(sample_count)
    half_width = T_CRITICAL_95[degrees_of_freedom] * standard_error
    return {
        "confidence_level": 0.95,
        "distribution": "student-t",
        "sample_count": sample_count,
        "degrees_of_freedom": degrees_of_freedom,
        "mean_ns": mean,
        "standard_deviation_ns": standard_deviation,
        "standard_error_ns": standard_error,
        "half_width_ns": half_width,
        "lower_ns": mean - half_width,
        "upper_ns": mean + half_width,
    }


def summarize_samples(
    samples: Sequence[Dict[str, Any]], measured_runs: int = 12
) -> Dict[str, Any]:
    if len(samples) not in (1, measured_runs):
        raise HarnessError(
            f"sample count must be one slow-run result or {measured_runs}, "
            f"found {len(samples)}"
        )
    wall_values = [int(sample["wall_ns"]) for sample in samples]
    rss_values = [int(sample["peak_rss_bytes"]) for sample in samples]
    if len(samples) == 1:
        return {
            "measurement_mode": "single-run-over-slow-cutoff",
            "runs": 1,
            "wall_min_ns": wall_values[0],
            "wall_median_ns": wall_values[0],
            "wall_iqr_ns": None,
            "cpu_statistics_exclude_first_run": True,
            "cpu_mean_ns": None,
            "cpu_ci_95": None,
            "peak_rss_median_bytes": rss_values[0],
            "peak_rss_max_bytes": rss_values[0],
        }
    quartiles = statistics.quantiles(wall_values, n=4, method="inclusive")
    cpu_values = [int(sample["cpu_total_ns"]) for sample in samples[1:]]
    cpu_ci = confidence_interval_95(cpu_values)
    return {
        "measurement_mode": "twelve-run",
        "runs": measured_runs,
        "wall_min_ns": min(wall_values),
        "wall_median_ns": statistics.median(wall_values),
        "wall_iqr_ns": quartiles[2] - quartiles[0],
        "wall_q1_ns": quartiles[0],
        "wall_q3_ns": quartiles[2],
        "cpu_statistics_exclude_first_run": True,
        "cpu_sample_count": len(cpu_values),
        "cpu_excluded_run_indices": [1],
        "cpu_mean_ns": statistics.fmean(cpu_values),
        "cpu_ci_95": cpu_ci,
        "peak_rss_median_bytes": statistics.median(rss_values),
        "peak_rss_max_bytes": max(rss_values),
    }


def correctness_gate(
    manifest: Dict[str, Any],
    suite_root: Path,
    binaries_by_workload: Dict[str, Dict[str, Path]],
) -> List[Dict[str, Any]]:
    results = []
    timeout_seconds = float(manifest["methodology"]["correctness_timeout_seconds"])
    for workload in manifest["workloads"]:
        workload_id = workload["id"]
        fixture_path = resolve_suite_path(
            suite_root,
            workload["fixtures"]["correctness"]["path"],
            f"{workload_id} correctness fixture",
        )
        expected = fixture_path.read_bytes()
        implementations = {}
        for implementation in IMPLEMENTATIONS:
            command = [
                str(binaries_by_workload[workload_id][implementation]),
                workload["correctness_input"],
            ]
            environment = {"GOMAXPROCS": "1"} if implementation == "go" else {}
            sample, stdout = timed_run(
                command,
                expected_stdout=expected,
                timeout_seconds=timeout_seconds,
                environment_overrides=environment,
            )
            implementations[implementation] = {
                "passed": True,
                "stdout_sha256": sha256_bytes(stdout or b""),
                "sample": sample,
            }
        results.append(
            {
                "id": workload_id,
                "input": workload["correctness_input"],
                "fixture_path": str(fixture_path),
                "fixture_sha256": sha256_file(fixture_path),
                "implementations": implementations,
            }
        )
    return results


def measure_workload(
    manifest: Dict[str, Any],
    suite_root: Path,
    workload: Dict[str, Any],
    binaries: Dict[str, Path],
    workload_index: int,
) -> Dict[str, Any]:
    methodology = manifest["methodology"]
    measured_runs = int(methodology["measured_runs"])
    slow_cutoff_ns = int(float(methodology["slow_cutoff_seconds"]) * 1_000_000_000)
    hard_timeout_seconds = float(methodology["hard_timeout_seconds"])
    seed = int(methodology["random_seed"]) + workload_index
    generator = random.Random(seed)
    fixture_path = resolve_suite_path(
        suite_root,
        workload["fixtures"]["performance"]["path"],
        f"{workload['id']} performance fixture",
    )
    expected = fixture_path.read_bytes()
    samples = {implementation: [] for implementation in IMPLEMENTATIONS}
    first_outputs = {}
    single_run = set()
    execution_order = []
    for round_index in range(measured_runs):
        order = list(IMPLEMENTATIONS)
        generator.shuffle(order)
        round_record = {
            "round_index": round_index + 1,
            "shuffled_order": order,
            "executed_order": [],
            "skipped_single_run_downgrades": [],
        }
        execution_order.append(round_record)
        for order_position, implementation in enumerate(order):
            if implementation in single_run:
                round_record["skipped_single_run_downgrades"].append(implementation)
                continue
            round_record["executed_order"].append(implementation)
            command = [
                str(binaries[implementation]),
                workload["performance_input"],
            ]
            first_run = len(samples[implementation]) == 0
            environment = {"GOMAXPROCS": "1"} if implementation == "go" else {}
            sample, stdout = timed_run(
                command,
                expected_stdout=expected if first_run else None,
                timeout_seconds=hard_timeout_seconds,
                environment_overrides=environment,
            )
            sample["run_index"] = len(samples[implementation]) + 1
            sample["round_index"] = round_index + 1
            sample["order_position"] = order_position + 1
            samples[implementation].append(sample)
            if first_run:
                first_outputs[implementation] = {
                    "verified": True,
                    "sha256": sha256_bytes(stdout or b""),
                    "text": (stdout or b"").decode("utf-8"),
                }
                if sample["wall_ns"] >= slow_cutoff_ns:
                    single_run.add(implementation)

    implementations = {}
    for implementation in IMPLEMENTATIONS:
        summary = summarize_samples(
            samples[implementation], measured_runs=measured_runs
        )
        implementations[implementation] = {
            "single_run_downgrade": implementation in single_run,
            "slow_cutoff_seconds": methodology["slow_cutoff_seconds"],
            "first_formal_output": first_outputs[implementation],
            "samples": samples[implementation],
            "summary": summary,
        }
    nomo_min = implementations["nomo"]["summary"]["wall_min_ns"]
    go_min = implementations["go"]["summary"]["wall_min_ns"]
    c_min = implementations["c"]["summary"]["wall_min_ns"]
    return {
        "id": workload["id"],
        "performance_input": workload["performance_input"],
        "fixture_path": str(fixture_path),
        "fixture_sha256": sha256_file(fixture_path),
        "rotation_seed": seed,
        "execution_order_by_round": execution_order,
        "implementations": implementations,
        "relative_time_vs_go": nomo_min / go_min,
        "relative_time_vs_c": nomo_min / c_min,
        "relative_time_interpretation": (
            "Only a value below 1.0 means Nomo was faster for this workload."
        ),
    }


def system_capture(command: Sequence[str]) -> Optional[str]:
    try:
        _, stdout, _ = run_capture(command, timeout_seconds=10.0)
    except HarnessError:
        return None
    value = stdout.decode("utf-8", errors="replace").strip()
    return value or None


def parse_macos_hardware_profile(
    profile: str,
) -> Tuple[Optional[str], Optional[int]]:
    cpu_model = None
    physical_cores = None
    for raw_line in profile.splitlines():
        key, separator, value = raw_line.strip().partition(":")
        if not separator:
            continue
        if key == "Chip":
            cpu_model = value.strip() or None
        elif key == "Total Number of Cores":
            core_count = value.strip().split(maxsplit=1)[0]
            if core_count.isdigit():
                physical_cores = int(core_count)
    return cpu_model, physical_cores


def host_provenance() -> Dict[str, Any]:
    cpu_model = platform.processor() or None
    physical_cores = None
    if sys.platform == "darwin":
        cpu_model = system_capture(["sysctl", "-n", "machdep.cpu.brand_string"])
        if cpu_model is None:
            cpu_model = system_capture(["sysctl", "-n", "hw.model"])
        physical_text = system_capture(["sysctl", "-n", "hw.physicalcpu"])
        if physical_text and physical_text.isdigit():
            physical_cores = int(physical_text)
        if cpu_model is None or physical_cores is None:
            profile = system_capture(
                [
                    "/usr/sbin/system_profiler",
                    "SPHardwareDataType",
                    "-detailLevel",
                    "mini",
                ]
            )
            if profile is not None:
                profile_cpu, profile_cores = parse_macos_hardware_profile(profile)
                cpu_model = cpu_model or profile_cpu
                physical_cores = physical_cores or profile_cores
    elif Path("/proc/cpuinfo").is_file():
        for line in Path("/proc/cpuinfo").read_text(
            encoding="utf-8", errors="replace"
        ).splitlines():
            if line.lower().startswith("model name") and ":" in line:
                cpu_model = line.split(":", 1)[1].strip()
                break
    return {
        "os": platform.system(),
        "os_release": platform.release(),
        "os_version": platform.version(),
        "platform": platform.platform(),
        "architecture": platform.machine(),
        "cpu_model": cpu_model,
        "physical_core_count": physical_cores,
        "logical_core_count": os.cpu_count(),
    }


def validate_compile_command_provenance(
    builds: Dict[str, Dict[str, Any]]
) -> None:
    for workload_id, build in builds.items():
        commands = build.get("commands", {})
        if set(commands) != {
            "nomo_emit_c",
            "nomo_clang",
            "c_clang",
            "go_build",
        }:
            raise HarnessError(f"{workload_id} build command provenance is incomplete")
        for name, record in commands.items():
            argv = record.get("argv")
            rendered = record.get("command")
            if not isinstance(argv, list) or not argv or rendered != command_text(argv):
                raise HarnessError(
                    f"{workload_id} {name} command provenance is not executable"
                )
        for name in ("nomo_clang", "c_clang"):
            argv = commands[name]["argv"]
            if tuple(argv[1:5]) != BASE_CLANG_FLAGS:
                raise HarnessError(
                    f"{workload_id} {name} lost the fixed Clang flags"
                )


def validate_result(result: Dict[str, Any]) -> None:
    required = {
        "schema",
        "suite",
        "manifest_version",
        "mode",
        "created_at_utc",
        "claims",
        "provenance",
        "builds",
        "correctness",
        "workloads",
    }
    missing = sorted(required.difference(result))
    if missing:
        raise HarnessError("result is missing fields: " + ", ".join(missing))
    if result.get("schema") != 1:
        raise HarnessError("unsupported Benchmarks Game result schema")
    if result.get("suite") != "nomo-benchmarksgame-cpu-baseline":
        raise HarnessError("result suite identity mismatch")
    claims = result.get("claims", {})
    if claims != {
        "exploratory": True,
        "affinity_enforced": False,
        "claim_eligible": False,
        "scope": (
            "single-thread CPU, Array/COW behavior, floating point, "
            "and C99 code generation only"
        ),
    }:
        raise HarnessError("result must remain exploratory and claim-ineligible")
    validate_compile_command_provenance(result["builds"])
    if len(result["correctness"]) != 3:
        raise HarnessError("result must contain all three correctness gates")
    if result["mode"] == "measure":
        if len(result["workloads"]) != 3:
            raise HarnessError("formal result must contain all three measurements")
        for workload in result["workloads"]:
            implementations = workload.get("implementations", {})
            if set(implementations) != set(IMPLEMENTATIONS):
                raise HarnessError(
                    f"{workload.get('id')} result is missing implementations"
                )
            for implementation in IMPLEMENTATIONS:
                entry = implementations[implementation]
                sample_count = len(entry.get("samples", []))
                downgrade = entry.get("single_run_downgrade")
                if downgrade and sample_count != 1:
                    raise HarnessError("slow-run downgrade must contain one sample")
                if not downgrade and sample_count != 12:
                    raise HarnessError("normal measurement must contain twelve samples")
            for ratio_name in ("relative_time_vs_go", "relative_time_vs_c"):
                ratio = workload.get(ratio_name)
                if not isinstance(ratio, (int, float)) or ratio <= 0:
                    raise HarnessError(f"{workload.get('id')} has invalid {ratio_name}")


def default_output_path(mode: str) -> Path:
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return DEFAULT_RESULTS_ROOT / f"{mode}-{timestamp}.json"


def write_result(path: Path, result: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def run_suite(arguments: argparse.Namespace) -> Tuple[Path, Dict[str, Any]]:
    manifest_path = Path(arguments.manifest).resolve()
    manifest = read_json(manifest_path)
    suite_root = manifest_path.parent
    validate_manifest(manifest, suite_root)
    output_path = (
        Path(arguments.output).resolve()
        if arguments.output
        else default_output_path(arguments.mode).resolve()
    )
    bundle_root = output_path.with_suffix("")
    bundle_root.mkdir(parents=True, exist_ok=True)

    repository = repository_state(
        REPOSITORY_ROOT, require_clean=arguments.require_clean
    )
    toolchains = inspect_toolchains(
        manifest,
        nomo_argument=Path(arguments.nomo),
        clang_argument=arguments.clang,
        go_argument=arguments.go,
    )
    builds = {}
    binaries_by_workload = {}
    for workload in manifest["workloads"]:
        build, binaries = build_workload(
            workload,
            suite_root,
            bundle_root,
            toolchains,
            build_timeout_seconds=float(
                manifest["methodology"]["build_timeout_seconds"]
            ),
        )
        builds[workload["id"]] = build
        binaries_by_workload[workload["id"]] = binaries

    correctness = correctness_gate(manifest, suite_root, binaries_by_workload)
    measurements = []
    if arguments.mode == "measure":
        for workload_index, workload in enumerate(manifest["workloads"]):
            measurements.append(
                measure_workload(
                    manifest,
                    suite_root,
                    workload,
                    binaries_by_workload[workload["id"]],
                    workload_index,
                )
            )

    result = {
        "schema": 1,
        "suite": manifest["suite"],
        "manifest_version": manifest["manifest_version"],
        "mode": arguments.mode,
        "created_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "claims": {
            "exploratory": True,
            "affinity_enforced": False,
            "claim_eligible": False,
            "scope": (
                "single-thread CPU, Array/COW behavior, floating point, "
                "and C99 code generation only"
            ),
        },
        "provenance": {
            "repository": repository,
            "manifest_path": str(manifest_path),
            "manifest_sha256": sha256_file(manifest_path),
            "host": host_provenance(),
            "toolchains": toolchains,
            "methodology": manifest["methodology"],
            "methodology_urls": manifest["methodology_urls"],
        },
        "builds": builds,
        "correctness": correctness,
        "workloads": measurements,
    }
    validate_result(result)
    write_result(output_path, result)
    return output_path, result


def parse_arguments(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        default=str(DEFAULT_MANIFEST),
        help="versioned suite manifest",
    )
    parser.add_argument(
        "--nomo",
        default=str(REPOSITORY_ROOT / "target" / "release" / "nomo"),
        help="Nomo compiler driver",
    )
    parser.add_argument("--clang", default="clang", help="Clang executable")
    parser.add_argument("--go", default="go", help="Go executable")
    parser.add_argument(
        "--mode",
        choices=("correctness", "measure"),
        default="correctness",
    )
    parser.add_argument("--output", help="result JSON path")
    parser.add_argument(
        "--require-clean",
        action="store_true",
        help="reject a dirty Git checkout",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    try:
        arguments = parse_arguments(argv)
        output_path, result = run_suite(arguments)
    except HarnessError as error:
        print(f"benchmarksgame: {error}", file=sys.stderr)
        return 1
    print(f"wrote {output_path}")
    for item in result["correctness"]:
        print(f"correctness {item['id']}: Nomo/C/Go match")
    for item in result["workloads"]:
        print(
            f"{item['id']}: "
            f"Nomo/Go={item['relative_time_vs_go']:.6f} "
            f"Nomo/C={item['relative_time_vs_c']:.6f}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
