#!/usr/bin/env python3
"""RFC 0043 Benchmarks Game parity harness with fail-closed release lanes."""

from __future__ import annotations

import argparse
import base64
import binascii
import contextlib
import ctypes
import datetime as dt
import json
import math
import os
import platform
import re
import shutil
import signal
import stat
import statistics
import subprocess
import sys
import tempfile
import time
import unicodedata
import uuid
from pathlib import Path, PurePath, PurePosixPath, PureWindowsPath
from typing import Any, Callable, Dict, Iterator, Optional, Sequence, Tuple
from ctypes import wintypes

import benchmarksgame as v1

try:
    import tomllib
except ImportError:
    import tomli as tomllib  # type: ignore[no-redef]

try:
    from jsonschema import Draft202012Validator, FormatChecker
except ImportError:
    Draft202012Validator = None  # type: ignore[assignment,misc]
    FormatChecker = None  # type: ignore[assignment,misc]


def _strict_json_object(pairs: Sequence[Tuple[str, Any]]) -> Dict[str, Any]:
    result: Dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise HarnessError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def read_json_strict(path: Path) -> Dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as source:
            value = json.load(source, object_pairs_hook=_strict_json_object)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise HarnessError(f"cannot read strict JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise HarnessError(f"strict JSON root must be an object: {path}")
    return value


def write_canonical_json(path: Path, value: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(
        (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    )
    temporary.replace(path)


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    REPOSITORY_ROOT / "performance" / "benchmarksgame" / "manifest-v2.json"
)
DEFAULT_RESULTS_ROOT = (
    REPOSITORY_ROOT / "performance" / "results" / "benchmarksgame-v2"
)
WORKLOAD_IDS = ("spectral-norm", "n-body", "fannkuch-redux")
FORMAL_BUILD_MODES = ("release", "emit-c")
TIMED_LANES = ("candidate", "main", "c", "cpp", "go")
REFERENCE_LANES = ("c", "cpp", "semantic-c", "go")
CORRECTNESS_BASELINE_LANES = ("nomo-baseline", *REFERENCE_LANES)
CORRECTNESS_FORMAL_LANES = ("candidate", "main", *REFERENCE_LANES)
REFERENCE_BUILD_COMMANDS = (
    "c_build",
    "cpp_build",
    "semantic-c_build",
    "go_build",
)
NOMO_BASELINE_BUILD_COMMANDS = (
    "nomo_baseline_emit_c",
    "nomo_baseline_clang",
)
DECISIVE_COMPARATORS = ("c", "cpp", "main")
DIAGNOSTIC_COMPARATORS = ("go",)
FORBIDDEN_COMPILER_FLAG_PREFIXES = (
    "-ffast-math",
    "-Ofast",
    "-flto",
    "-fprofile",
    "-march=",
    "-mcpu=",
    "-mtune=",
    "-mavx",
    "-msse",
    "-mfpu",
    "-msimd",
)
BASE_C_FLAGS = ("-std=c99", "-O3", "-DNDEBUG", "-fomit-frame-pointer")
BASE_CPP_FLAGS = (
    "-std=c++20",
    "-pedantic-errors",
    "-O3",
    "-DNDEBUG",
    "-fomit-frame-pointer",
)
CLANG_DRIVER_CONFIG_FLAGS = ("--no-default-config",)
GO_BUILD_ENVIRONMENT_CONTRACT = {
    "GOENV": "off",
    "GOCACHE": "isolated-empty-per-build-removed-after-build",
    "GOMODCACHE": "isolated-empty-per-build-removed-after-build",
}
GO_BUILD_CACHE_DIRECTORIES = {
    "GOCACHE": "go-build-cache",
    "GOMODCACHE": "go-module-cache",
}
T_CRITICAL_99_DF29 = 2.462021360150384
STDOUT_NORMALIZATION = "crlf-to-lf-only-v1"
EXPECTED_V1_MANIFEST_SHA = (
    "bd8e5016fb376741478806d13585ebc37ade2104995bd411a2a161592f65c15f"
)
EXPECTED_V2_MANIFEST_SHA = (
    "aeb4fb31a467378754bbae32b13e8aeb829184a3e40953704e11a18bdbac7115"
)
EXPECTED_REQUIRED_CHECKS = (
    "canonical_host_identity",
    "os_kernel",
    "architecture",
    "cpu_model_topology",
    "memory",
    "power_mode",
    "frequency_governor",
    "thermal_state",
    "virtualization",
    "clock_source_resolution",
    "affinity_isolation",
    "concurrent_load",
    "toolchain_identity",
    "frozen_source_lock",
)
DYNAMIC_ENVIRONMENT_POLICY = {
    "require_ac_power": True,
    "allow_low_power_mode": False,
    "allowed_linux_governors": ["performance"],
    "max_thermal_celsius": 85.0,
    "max_load_per_logical_core": 1.0,
    "max_swap_delta_bytes": 0,
}
DARWIN_PMSET = "/usr/bin/pmset"
DARWIN_OSASCRIPT = "/usr/bin/osascript"
DARWIN_SYSCTL = "/usr/sbin/sysctl"
DARWIN_THERMAL_STATE_SCRIPT = (
    'ObjC.import("Foundation"); '
    "$.NSProcessInfo.processInfo.thermalState"
)
DARWIN_PMSET_NO_RECORDED_LINES = (
    "Note: No thermal warning level has been recorded",
    "Note: No performance warning level has been recorded",
    "Note: No CPU power status has been recorded",
)
COMPILER_AFFECTING_ENVIRONMENT = (
    "CPATH",
    "C_INCLUDE_PATH",
    "CPLUS_INCLUDE_PATH",
    "OBJC_INCLUDE_PATH",
    "LIBRARY_PATH",
    "SDKROOT",
    "MACOSX_DEPLOYMENT_TARGET",
    "DEVELOPER_DIR",
    "TOOLCHAINS",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "CL",
    "_CL_",
    "CC",
    "CXX",
    "CFLAGS",
    "CPPFLAGS",
    "CXXFLAGS",
    "LDFLAGS",
    "GOFLAGS",
    "GOENV",
    "GOCACHE",
    "GOMODCACHE",
    "CGO_CFLAGS",
    "CGO_CPPFLAGS",
    "CGO_CXXFLAGS",
    "CGO_FFLAGS",
    "CGO_LDFLAGS",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_HOME",
)
BUILD_ENVIRONMENT_WHITELIST = (
    "PATH",
    "HOME",
    "USERPROFILE",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SystemRoot",
    "WINDIR",
    "RUSTUP_HOME",
)
WINDOWS_CANONICAL_PATHEXT = (".COM", ".EXE", ".BAT", ".CMD")
EXPECTED_ALLOCATION_MAPPING_SHA256 = {
    "spectral-norm": "21f5db296a9a67d542cca32aaf63abe8e2aba59efc064d7cc2c48b9f42a6bca0",
    "n-body": "2cb305ee6cdbc10735c94b16b4ad88c3215adec8cc77f76aff0253f0033514c8",
    "fannkuch-redux": "a6cc2c5404dd323ab500c82fb8d1850933412d6204aadcdf3344d1cde073b9e7",
}
_WINDOWS_BUILD_SUPPORT_CACHE: Optional[Dict[str, Any]] = None
_DARWIN_BUILD_SUPPORT_CACHE: Optional[Dict[str, Any]] = None

HarnessError = v1.HarnessError
ToolchainMismatch = v1.ToolchainMismatch
CommandTimedOut = v1.CommandTimedOut


class SampleCollectionError(HarnessError):
    def __init__(self, message: str, record: Dict[str, Any]) -> None:
        super().__init__(message)
        self.record = record


class SampleTimeoutError(SampleCollectionError, CommandTimedOut):
    pass


class BuildCollectionError(HarnessError):
    def __init__(self, message: str, record: Dict[str, Any]) -> None:
        super().__init__(message)
        self.record = record


class WorkloadBuildError(HarnessError):
    def __init__(self, message: str, record: Dict[str, Any]) -> None:
        super().__init__(message)
        self.record = record


def stable_build_path() -> str:
    home = Path.home().resolve()
    candidates = [
        Path("/opt/homebrew/bin"),
        Path("/usr/local/bin"),
        home / ".cargo" / "bin",
        home / "go" / "bin",
        Path("/Applications/Xcode.app/Contents/Developer/usr/bin"),
        Path("/Library/Developer/CommandLineTools/usr/bin"),
        Path("/usr/bin"),
        Path("/bin"),
        Path("/usr/sbin"),
        Path("/sbin"),
        Path("/snap/bin"),
    ]
    if os.name == "nt":
        system_root = windows_system_directory().parent
        system_drive = Path(system_root.anchor)
        candidates = [
            home / ".cargo" / "bin",
            system_drive / "Program Files" / "LLVM" / "bin",
            system_drive / "Program Files" / "Go" / "bin",
            system_root / "System32",
            system_root,
            system_root / "System32" / "Wbem",
            system_root / "System32" / "WindowsPowerShell" / "v1.0",
        ]

    entries = []
    for candidate in candidates:
        entry = candidate.expanduser().resolve()
        normalized = str(entry).replace("\\", "/").lower()
        if "/.codex/tmp/arg0/" in f"{normalized}/":
            continue
        value = str(entry)
        if entry.is_dir() and value not in entries:
            entries.append(value)
    if not entries:
        raise HarnessError("controlled build PATH has no stable directories")
    return os.pathsep.join(entries)


def sanitized_build_environment(
    approved_overrides: Optional[Dict[str, str]] = None,
) -> Tuple[Dict[str, str], Dict[str, Any]]:
    windows_toolchain = None
    darwin_sdk = None
    if os.name == "nt":
        support = canonical_windows_build_support()
        environment = dict(support["environment"])
        windows_toolchain = support["authority"]
        for key in ("HOME", "USERPROFILE", "RUSTUP_HOME"):
            if value := os.environ.get(key):
                environment[key] = value
    else:
        environment = {
            key: value
            for key in BUILD_ENVIRONMENT_WHITELIST
            if key != "PATH"
            if (value := os.environ.get(key))
        }
        if platform.system() == "Darwin":
            darwin_sdk = canonical_darwin_build_support()
            environment["SDKROOT"] = darwin_sdk["sdkroot"]
    environment["GOENV"] = "off"
    if os.name != "nt":
        environment["PATH"] = stable_build_path()
    overrides = approved_overrides or {}
    unexpected = set(overrides) - {
        "CARGO_TARGET_DIR",
        "CARGO_HOME",
        "RUSTC",
        "GOCACHE",
        "GOMODCACHE",
    }
    if unexpected:
        raise HarnessError(
            "unapproved build environment overrides: "
            + ", ".join(sorted(unexpected))
        )
    environment.update(overrides)
    environment.update({"LC_ALL": "C", "LANG": "C"})
    projection = {
        "retained": {
            key: str(Path(value).resolve())
            if key
            in {
                "HOME",
                "USERPROFILE",
                "TMPDIR",
                "TEMP",
                "TMP",
                "SystemRoot",
                "WINDIR",
                "CARGO_HOME",
                "RUSTUP_HOME",
                "CARGO_TARGET_DIR",
                "RUSTC",
                "GOCACHE",
                "GOMODCACHE",
            }
            else value
            for key, value in sorted(environment.items())
        },
        "cleared": [
            name
            for name in COMPILER_AFFECTING_ENVIRONMENT
            if name not in environment
        ],
        "cleared_values_recorded": False,
    }
    if windows_toolchain is not None:
        projection["windows_toolchain"] = windows_toolchain
    if darwin_sdk is not None:
        projection["darwin_sdk"] = darwin_sdk
    return environment, projection


@contextlib.contextmanager
def isolated_go_build_cache(
    build_root: Path,
) -> Iterator[Dict[str, str]]:
    cache_paths = {
        key: (build_root / directory).resolve()
        for key, directory in GO_BUILD_CACHE_DIRECTORIES.items()
    }
    existing = [str(path) for path in cache_paths.values() if path.exists()]
    if existing:
        raise HarnessError(
            "isolated Go build cache was not empty at creation: "
            + ", ".join(existing)
        )
    for path in cache_paths.values():
        path.mkdir(parents=True)
    try:
        yield {key: str(path) for key, path in cache_paths.items()}
    finally:
        for path in cache_paths.values():
            if path.exists():
                shutil.rmtree(path)


def validate_build_command_environment(
    record: Dict[str, Any],
    label: str,
    approved_overrides: Optional[Dict[str, str]] = None,
    authority_projection: Optional[Dict[str, Any]] = None,
) -> None:
    if authority_projection is None:
        expected = sanitized_build_environment(approved_overrides)[1]
    else:
        expected = json.loads(json.dumps(authority_projection))
        retained = expected.get("retained")
        cleared = expected.get("cleared")
        if (
            not isinstance(retained, dict)
            or not isinstance(cleared, list)
            or expected.get("cleared_values_recorded") is not False
        ):
            raise HarnessError(
                f"{label} build environment authority is incomplete"
            )
        for key, value in (approved_overrides or {}).items():
            retained[key] = value
            if key in cleared:
                cleared.remove(key)
        expected["retained"] = {
            key: retained[key] for key in sorted(retained)
        }
        expected["cleared"] = [
            name
            for name in COMPILER_AFFECTING_ENVIRONMENT
            if name not in expected["retained"]
        ]
    if record.get("environment") != expected:
        raise HarnessError(
            f"{label} build environment differs from the canonical sanitized projection"
        )


def run_build_capture(
    command: Sequence[str],
    timeout_seconds: float,
    cwd: Optional[Path] = None,
    approved_environment_overrides: Optional[Dict[str, str]] = None,
) -> Tuple[Dict[str, Any], bytes, bytes]:
    environment, projection = sanitized_build_environment(
        approved_environment_overrides
    )
    argv = [str(part) for part in command]
    started_at_utc = dt.datetime.now(dt.timezone.utc).isoformat()
    started = time.perf_counter_ns()
    try:
        completed = subprocess.run(
            argv,
            cwd=str(cwd) if cwd is not None else None,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout or b""
        stderr = error.stderr or b""
        record = failed_build_command_record(
            argv,
            cwd,
            projection,
            started_at_utc,
            time.perf_counter_ns() - started,
            stdout,
            stderr,
            exit_code=None,
            timed_out=True,
            error=f"command exceeded {timeout_seconds:.3f}s",
        )
        raise BuildCollectionError(
            f"command exceeded {timeout_seconds:.3f}s: {v1.command_text(argv)}",
            record,
        ) from error
    except OSError as error:
        record = failed_build_command_record(
            argv,
            cwd,
            projection,
            started_at_utc,
            time.perf_counter_ns() - started,
            b"",
            b"",
            exit_code=None,
            timed_out=False,
            error=f"{type(error).__name__}: {error}",
        )
        raise BuildCollectionError(
            f"could not launch build command: {v1.command_text(argv)}: {error}",
            record,
        ) from error
    duration_ns = time.perf_counter_ns() - started
    if completed.returncode != 0:
        record = failed_build_command_record(
            argv,
            cwd,
            projection,
            started_at_utc,
            duration_ns,
            completed.stdout,
            completed.stderr,
            exit_code=completed.returncode,
            timed_out=False,
            error=f"command exited with status {completed.returncode}",
        )
        raise BuildCollectionError(
            f"command failed with exit {completed.returncode}: "
            f"{v1.command_text(argv)}\n"
            "stdout:\n"
            f"{completed.stdout.decode('utf-8', errors='replace')}\n"
            "stderr:\n"
            f"{completed.stderr.decode('utf-8', errors='replace')}",
            record,
        )
    record = {
        "argv": argv,
        "command": v1.command_text(argv),
        "cwd": str(cwd.resolve()) if cwd is not None else None,
        "duration_ns": duration_ns,
        "exit_code": 0,
    }
    record["environment"] = projection
    return record, completed.stdout, completed.stderr


def failed_build_command_record(
    argv: Sequence[str],
    cwd: Optional[Path],
    environment: Dict[str, Any],
    started_at_utc: str,
    duration_ns: int,
    stdout: bytes,
    stderr: bytes,
    *,
    exit_code: Optional[int],
    timed_out: bool,
    error: str,
) -> Dict[str, Any]:
    return {
        "argv": list(argv),
        "command": v1.command_text(argv),
        "cwd": str(cwd.resolve()) if cwd is not None else None,
        "duration_ns": duration_ns,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "started_at_utc": started_at_utc,
        "finished_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "environment": environment,
        "stdout": stdout.decode("utf-8", errors="replace"),
        "stderr": stderr.decode("utf-8", errors="replace"),
        "error": error,
    }


def workload_build_failure_record(
    workload_id: str,
    lane: str,
    phase: str,
    source: Path,
    output: Path,
    command: Dict[str, Any],
    failure_kind: str = "command-failure",
) -> Dict[str, Any]:
    output_path = canonical_build_output_path(output)
    return {
        "kind": "preflight-build-failure-v1",
        "failure_kind": failure_kind,
        "workload_id": workload_id,
        "lane": lane,
        "phase": phase,
        "source": {
            "path": str(source.resolve()),
            "sha256": v1.sha256_file(source),
        },
        "output_path": str(output_path),
        "output_state": build_output_state(output_path),
        "command": command,
    }


def build_output_state(path: Path) -> Dict[str, Any]:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return {
            "kind": "absent",
            "lstat_type": "absent",
            "symlink_target": None,
        }
    mode = metadata.st_mode
    if stat.S_ISREG(mode):
        kind = "regular"
        lstat_type = "regular"
    elif stat.S_ISDIR(mode):
        kind = "directory"
        lstat_type = "directory"
    elif stat.S_ISLNK(mode):
        kind = "symlink"
        lstat_type = "symlink"
    else:
        kind = "other-nonregular"
        if stat.S_ISFIFO(mode):
            lstat_type = "fifo"
        elif stat.S_ISSOCK(mode):
            lstat_type = "socket"
        elif stat.S_ISCHR(mode):
            lstat_type = "character-device"
        elif stat.S_ISBLK(mode):
            lstat_type = "block-device"
        else:
            lstat_type = "unknown"
    return {
        "kind": kind,
        "lstat_type": lstat_type,
        "symlink_target": os.readlink(path) if kind == "symlink" else None,
    }


def canonical_build_output_path(path: Path) -> Path:
    return path.parent.resolve() / path.name


def build_output_failure_error(
    expected_output: Path,
    output_state: Dict[str, Any],
) -> str:
    path = canonical_build_output_path(expected_output)
    if output_state["kind"] == "absent":
        return f"expected build output is missing: {path}"
    return (
        "expected build output is not a regular file "
        f"({output_state['lstat_type']}): {path}"
    )


def recorded_build_output_failure_error(
    output_path: str,
    output_state: Dict[str, Any],
) -> str:
    if output_state["kind"] == "absent":
        return f"expected build output is missing: {output_path}"
    return (
        "expected build output is not a regular file "
        f"({output_state['lstat_type']}): {output_path}"
    )


def failed_build_output_command_record(
    command: Dict[str, Any],
    stdout: bytes,
    stderr: bytes,
    expected_output: Path,
    output_state: Dict[str, Any],
) -> Dict[str, Any]:
    finished = dt.datetime.now(dt.timezone.utc)
    duration_ns = int(command["duration_ns"])
    started = finished - dt.timedelta(microseconds=duration_ns / 1_000)
    return {
        **command,
        "timed_out": False,
        "started_at_utc": started.isoformat(),
        "finished_at_utc": finished.isoformat(),
        "stdout": stdout.decode("utf-8", errors="replace"),
        "stderr": stderr.decode("utf-8", errors="replace"),
        "error": build_output_failure_error(expected_output, output_state),
    }


def require_regular_workload_build_output(
    *,
    workload_id: str,
    lane: str,
    phase: str,
    source: Path,
    output: Path,
    command: Dict[str, Any],
    stdout: bytes,
    stderr: bytes,
) -> None:
    output_state = build_output_state(output)
    if output_state["kind"] == "regular":
        return
    failure_kind = (
        "missing-output"
        if output_state["kind"] == "absent"
        else "invalid-output"
    )
    message = build_output_failure_error(output, output_state)
    raise WorkloadBuildError(
        message,
        workload_build_failure_record(
            workload_id,
            lane,
            phase,
            source,
            output,
            failed_build_output_command_record(
                command,
                stdout,
                stderr,
                output,
                output_state,
            ),
            failure_kind,
        ),
    )


def resolve_executable(value: str, label: str) -> Path:
    resolved = None
    if (
        platform.system() == "Darwin"
        and value in {"clang", "clang++"}
        and (Path("/usr/bin") / value).is_file()
    ):
        resolved = str(Path("/usr/bin") / value)
    if resolved is None:
        if os.name == "nt":
            selected = (
                _windows_executable_from_path(value, stable_build_path())
                if Path(value).name == value
                else None
            )
            resolved = str(selected) if selected is not None else None
        else:
            resolved = shutil.which(value, path=stable_build_path())
    if resolved is None:
        candidate = Path(value).expanduser()
        candidates = [candidate]
        if os.name == "nt" and candidate.suffix.lower() != ".exe":
            candidates.append(candidate.with_suffix(".exe"))
        for path in candidates:
            if path.is_file():
                resolved = os.path.abspath(path)
                break
    if resolved is None:
        raise HarnessError(f"required {label} executable was not found: {value}")
    return Path(os.path.abspath(resolved))


def parse_rustc_verbose_version(value: str) -> Dict[str, str]:
    lines = [line.strip() for line in value.splitlines() if line.strip()]
    if not lines or not lines[0].startswith("rustc "):
        raise HarnessError("rustc -vV did not report a rustc release")
    fields: Dict[str, str] = {"version": lines[0]}
    for line in lines[1:]:
        key, separator, field_value = line.partition(":")
        if separator:
            normalized_key = re.sub(
                r"[-\s]+", "_", key.strip().lower()
            )
            fields[normalized_key] = field_value.strip()
    required = {
        "binary",
        "commit_hash",
        "commit_date",
        "host",
        "release",
        "llvm_version",
    }
    missing = sorted(required - set(fields))
    if missing:
        raise HarnessError(
            "rustc -vV is missing required fields: " + ", ".join(missing)
        )
    if fields["binary"] != "rustc":
        raise HarnessError("Rust compiler binary identity is not rustc")
    if re.fullmatch(r"[0-9a-f]{40}", fields["commit_hash"]) is None:
        raise HarnessError("Rust compiler commit hash is not full lowercase hex")
    return fields


def rustc_authority(
    rustc: Path,
    checkout: Path,
    approved_environment_overrides: Dict[str, str],
) -> Dict[str, Any]:
    invocation_path = Path(os.path.abspath(rustc))
    realpath = invocation_path.resolve()
    version_command, version_stdout, _ = run_build_capture(
        [str(invocation_path), "-vV"],
        30.0,
        cwd=checkout,
        approved_environment_overrides=approved_environment_overrides,
    )
    version_output = version_stdout.decode("utf-8", errors="strict").strip()
    version_fields = parse_rustc_verbose_version(version_output)
    sysroot_command, sysroot_stdout, _ = run_build_capture(
        [str(invocation_path), "--print", "sysroot"],
        30.0,
        cwd=checkout,
        approved_environment_overrides=approved_environment_overrides,
    )
    sysroot = sysroot_stdout.decode("utf-8", errors="strict").strip()
    sysroot_path = Path(sysroot)
    if not sysroot or not sysroot_path.is_absolute() or not sysroot_path.is_dir():
        raise HarnessError("rustc sysroot is not an absolute path")
    if invocation_path.parent.resolve() != (sysroot_path / "bin").resolve():
        raise HarnessError("rustc is not the actual compiler inside its sysroot")
    driver_files = sorted(
        (
            path.resolve()
            for path in sysroot_path.rglob("*rustc_driver*")
            if path.is_file()
        ),
        key=lambda path: str(path).casefold(),
    )
    if not driver_files:
        raise HarnessError("rustc sysroot has no rustc_driver artifact")
    return {
        "path": str(invocation_path),
        "realpath": str(realpath),
        "sha256": v1.sha256_file(realpath),
        "version_output": version_output,
        "version_fields": version_fields,
        "version_command": version_command,
        "sysroot": sysroot,
        "sysroot_command": sysroot_command,
        "toolchain": Path(sysroot).name,
        "driver_files": [
            {"path": str(path), "sha256": v1.sha256_file(path)}
            for path in driver_files
        ],
    }


def resolve_rustup_toolchain(
    cargo_invocation: Path,
    checkout: Path,
    approved_environment_overrides: Dict[str, str],
) -> Tuple[Path, Path, Dict[str, Any]]:
    invocation = Path(os.path.abspath(cargo_invocation))
    realpath = invocation.resolve()
    suffix = ".exe" if os.name == "nt" else ""
    sibling = invocation.with_name(f"rustup{suffix}")
    if (
        sibling.is_file()
        and v1.sha256_file(sibling.resolve()) == v1.sha256_file(realpath)
    ):
        rustup = Path(os.path.abspath(sibling))
    elif realpath.name.lower() in {"rustup", "rustup.exe"}:
        rustup = realpath
    else:
        raise HarnessError(
            "formal compiler build requires a rustup-managed Cargo invocation"
        )
    if not rustup.is_file():
        raise HarnessError(
            "formal compiler build requires a rustup-managed Cargo invocation"
        )
    commands = {}
    selected = {}
    for tool in ("cargo", "rustc"):
        record, stdout, _ = run_build_capture(
            [str(rustup), "which", tool],
            30.0,
            cwd=checkout,
            approved_environment_overrides=approved_environment_overrides,
        )
        path_text = stdout.decode("utf-8", errors="strict").strip()
        path = Path(path_text)
        if not path.is_absolute() or not path.is_file():
            raise HarnessError(f"rustup selected an unavailable {tool}: {path}")
        selected[tool] = path.resolve()
        commands[tool] = record
    cargo = selected["cargo"]
    rustc = selected["rustc"]
    if cargo.parent != rustc.parent or cargo.parent.name != "bin":
        raise HarnessError("rustup selected Cargo and rustc from different sysroots")
    return cargo, rustc, {
        "kind": "rustup-which-v1",
        "invocation_path": str(invocation),
        "rustup": {
            "path": str(rustup),
            "realpath": str(rustup.resolve()),
            "sha256": v1.sha256_file(rustup),
        },
        "cargo_command": commands["cargo"],
        "rustc_command": commands["rustc"],
        "selected_sysroot": str(cargo.parent.parent),
    }


def resolve_suite_path(suite_root: Path, relative: str, label: str) -> Path:
    return v1.resolve_suite_path(suite_root, relative, label)


def cargo_config_provenance(checkout: Path) -> list[Dict[str, str]]:
    checkout = checkout.resolve()
    records: list[Dict[str, str]] = []
    current = checkout
    while True:
        for name in ("config.toml", "config"):
            config_path = current / ".cargo" / name
            if not config_path.exists():
                continue
            if not config_path.is_file():
                raise HarnessError(
                    f"Cargo config is not a regular file: {config_path}"
                )
            resolved = config_path.resolve()
            try:
                relative = resolved.relative_to(checkout)
            except ValueError as error:
                raise HarnessError(
                    "external ancestor Cargo config is forbidden: "
                    f"{resolved}"
                ) from error
            relative_text = relative.as_posix()
            tracked = v1.git_capture(
                checkout,
                ["ls-files", "--error-unmatch", relative_text],
            )
            if tracked.strip() != relative_text:
                raise HarnessError(
                    f"Cargo config is not tracked by the exact checkout: "
                    f"{relative_text}"
                )
            try:
                config = tomllib.loads(
                    resolved.read_text(encoding="utf-8")
                )
            except (UnicodeError, tomllib.TOMLDecodeError) as error:
                raise HarnessError(
                    f"Cargo config is not valid UTF-8 TOML: {relative_text}"
                ) from error
            build = config.get("build", {})
            if not isinstance(build, dict):
                raise HarnessError(
                    f"Cargo config build table is invalid: {relative_text}"
                )
            forbidden = {
                "rustc",
                "rustc-wrapper",
                "rustc-workspace-wrapper",
            }
            if forbidden.intersection(build):
                raise HarnessError(
                    "tracked Cargo config may not replace the authority-bound "
                    f"Rust compiler or wrapper: {relative_text}"
                )
            records.append(
                {
                    "path": str(resolved),
                    "relative_path": relative_text,
                    "sha256": v1.sha256_file(resolved),
                }
            )
        parent = current.parent
        if parent == current:
            break
        current = parent
    return records


def validate_command_record(record: Dict[str, Any], label: str) -> None:
    argv = record.get("argv")
    rendered = record.get("command")
    if not isinstance(argv, list) or not argv:
        raise HarnessError(f"{label} command argv is missing")
    if rendered != v1.command_text(argv):
        raise HarnessError(f"{label} command rendering is not executable provenance")


def reject_forbidden_compiler_flags(argv: Sequence[str], label: str) -> None:
    for argument in argv:
        lowered = str(argument).lower()
        if any(
            lowered == prefix.lower() or lowered.startswith(prefix.lower())
            for prefix in FORBIDDEN_COMPILER_FLAG_PREFIXES
        ):
            raise HarnessError(f"{label} uses forbidden compiler flag: {argument}")


def validate_clang_driver_config_argv(
    argv: Sequence[str], label: str
) -> None:
    if tuple(
        argument
        for argument in argv
        if argument in CLANG_DRIVER_CONFIG_FLAGS
    ) != CLANG_DRIVER_CONFIG_FLAGS:
        raise HarnessError(
            f"{label} must contain exactly one canonical Clang "
            "--no-default-config flag"
        )
    for argument in argv:
        lowered = str(argument).lower()
        if (
            lowered.startswith("--config")
            or lowered.startswith("-config")
        ) and lowered not in {
            flag.lower() for flag in CLANG_DRIVER_CONFIG_FLAGS
        }:
            raise HarnessError(
                f"{label} may not load an external Clang driver config: "
                f"{argument}"
            )


def validate_json_schema(
    document: Dict[str, Any], schema_path: Path, label: str
) -> None:
    if Draft202012Validator is None or FormatChecker is None:
        raise HarnessError(
            "Draft 2020-12 validation requires "
            "python3 -m pip install -r scripts/requirements-benchmarksgame-v2.txt"
        )
    schema = read_json_strict(schema_path)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(
        validator.iter_errors(document), key=lambda error: list(error.path)
    )
    if errors:
        error = errors[0]
        location = ".".join(str(part) for part in error.absolute_path) or "<root>"
        raise HarnessError(
            f"{label} failed Draft 2020-12 schema at {location}: {error.message}"
        )


def validate_result_schema(result: Dict[str, Any], schema_path: Path) -> None:
    validate_json_schema(result, schema_path, "v2 result")


def validate_manifest(manifest: Dict[str, Any], suite_root: Path) -> None:
    if manifest.get("schema") != 2:
        raise HarnessError("unsupported Benchmarks Game v2 manifest schema")
    if manifest.get("suite") != "nomo-benchmarksgame-cpu-parity-v2":
        raise HarnessError("unexpected Benchmarks Game v2 suite identity")
    if manifest.get("manifest_version") != "2026-07-28":
        raise HarnessError("unexpected Benchmarks Game v2 manifest version")
    if manifest.get("license") != "BSD-3-Clause":
        raise HarnessError("v2 suite must preserve the BSD-3-Clause license")
    rfc = manifest.get("rfc", {})
    if rfc.get("number") != "0043" or rfc.get("merge_commit") != "b60f30c":
        raise HarnessError("v2 manifest must pin merged RFC 0043 at b60f30c")
    amendment_commit = rfc.get("allocation_clarification_merge_commit")
    if amendment_commit != "75a7e14adc1ea06ccdc9a28c1dc0676ce8404a1c":
        raise HarnessError("v2 manifest must pin the merged RFC allocation amendment")
    if rfc.get("delivery_blocked_pending_allocation_clarification") is not False:
        raise HarnessError("RFC allocation clarification delivery gate is still blocked")

    predecessor = manifest.get("predecessor", {})
    if predecessor.get("repository_commit") != (
        "c6712c1da1f65fcbdf0ce037224d11482b6a7e35"
    ):
        raise HarnessError("v2 predecessor must be frozen at nomo c6712c1")
    if predecessor.get("manifest_sha256") != EXPECTED_V1_MANIFEST_SHA:
        raise HarnessError("v2 predecessor manifest SHA is not the RFC freeze")
    v1_manifest = resolve_suite_path(
        suite_root,
        str(predecessor.get("manifest_path", "")),
        "v1 predecessor manifest",
    )
    if v1.sha256_file(v1_manifest) != EXPECTED_V1_MANIFEST_SHA:
        raise HarnessError("checked-in v1 manifest changed after the v2 freeze")

    methodology = manifest.get("methodology", {})
    required_methodology = {
        "formal_build_modes": list(FORMAL_BUILD_MODES),
        "timed_lanes": list(TIMED_LANES),
        "decisive_comparators": list(DECISIVE_COMPARATORS),
        "warmup_runs_per_lane": 2,
        "paired_blocks_per_batch": 30,
        "acceptance_batches": 2,
        "compile_time_in_run_time": False,
    }
    for key, expected in required_methodology.items():
        if methodology.get(key) != expected:
            raise HarnessError(f"v2 methodology {key} must be {expected!r}")
    statistics_contract = methodology.get("statistics", {})
    if statistics_contract.get("t_critical") != T_CRITICAL_99_DF29:
        raise HarnessError("v2 one-sided 99% t critical value changed")
    if statistics_contract.get("outlier_removal") is not False:
        raise HarnessError("v2 forbids outlier removal")
    if methodology.get("batch_invalidation") != {
        "decisive_reference_lanes": ["c", "cpp"],
        "diagnostic_reference_lanes": ["go"],
        "reference_drift_formula": (
            "max(geomean(blocks 16..30) / geomean(blocks 1..15), inverse) - 1"
        ),
        "reference_drift_max": 0.02,
        "paired_ratio_comparators": ["c", "cpp", "main"],
        "paired_ratio_rsd_formula": (
            "sample_stdev(candidate_wall / comparator_wall) / "
            "arithmetic_mean(candidate_wall / comparator_wall)"
        ),
        "paired_ratio_rsd_max": 0.03,
        "maximum_reruns": 1,
        "outlier_removal": False,
    }:
        raise HarnessError("v2 batch invalidation contract changed")
    schedule = methodology.get("schedule", {})
    if schedule.get("base_order_indices") != [0, 1, 4, 2, 3]:
        raise HarnessError("v2 Williams base sequence changed")
    generated_schedule = williams_schedule(TIMED_LANES, 30)
    validate_williams_schedule(generated_schedule, TIMED_LANES)

    thresholds = manifest.get("thresholds", {})
    if thresholds != {
        "inclusive": True,
        "workload": {
            "c_u99_max": 1.05,
            "cpp_u99_max": 1.05,
            "main_u99_max": 1.03,
        },
        "suite": {
            "c_point_max": 1.0,
            "c_u99_max": 1.03,
            "cpp_point_max": 1.0,
            "cpp_u99_max": 1.03,
            "main_u99_max": 1.02,
        },
    }:
        raise HarnessError("RFC 0043 acceptance thresholds changed")

    toolchains = manifest.get("toolchains", {})
    if toolchains.get("nomo", {}).get("release_command") != (
        "nomo build <project> --release"
    ):
        raise HarnessError("v2 requires the real nomo build --release contract")
    if toolchains.get("nomo", {}).get("formal_build_modes") != list(
        FORMAL_BUILD_MODES
    ):
        raise HarnessError("v2 requires independent release and emit-c protocols")
    if "--emit-c" not in str(toolchains.get("nomo", {}).get("emit_c_command", "")):
        raise HarnessError("v2 requires an explicit unmodified emit-c contract")
    if toolchains.get("nomo", {}).get("emit_c_fallback_allowed") is not False:
        raise HarnessError("v2 must not emulate release mode with --emit-c")
    clang = toolchains.get("clang", {})
    if tuple(clang.get("c_flags", [])) != BASE_C_FLAGS:
        raise HarnessError("v2 C flags changed")
    if tuple(clang.get("cpp_flags", [])) != BASE_CPP_FLAGS:
        raise HarnessError("v2 C++ flags changed")
    if tuple(clang.get("driver_config_flags", [])) != (
        CLANG_DRIVER_CONFIG_FLAGS
    ):
        raise HarnessError("v2 Clang driver config isolation changed")
    go = toolchains.get("go", {})
    if go.get("required_version") != "go1.25.12":
        raise HarnessError("v2 must pin the Go patch version")
    if go.get("build_environment") != GO_BUILD_ENVIRONMENT_CONTRACT:
        raise HarnessError(
            "v2 must disable external Go config and use isolated ephemeral caches"
        )

    required_checks = manifest.get("environment_qualification", {}).get(
        "required_checks"
    )
    if tuple(required_checks or []) != EXPECTED_REQUIRED_CHECKS:
        raise HarnessError(
            "v2 environment qualification checks must be the exact unique "
            "ordered RFC 0043 set"
        )

    workloads = manifest.get("workloads")
    if not isinstance(workloads, list):
        raise HarnessError("v2 workloads must be a list")
    if tuple(item.get("id") for item in workloads) != WORKLOAD_IDS:
        raise HarnessError("v2 must freeze exactly the three RFC workloads")
    frozen = {
        "spectral-norm": (
            "100",
            "5500",
            {
                "nomo": "f0caae510fbdc02d998a8c49275c4aca0b771642348286ce871515840f47fe30",
                "c": "1f7f71ce5fc6f87432b3801fb57c3e8a619da2527c1b801154b8102c7af66c3e",
                "cpp": "81489a76f22b02f67cd51f03753eaf45e05c459a08c1c69e50d6812a9acdd4b2",
                "semantic-c": "6951782dd59f50eb9eca1163b59d925fcda3bb8a88bdc7e722a43153fa5c1dc3",
                "go": "862a4ca6a79a7457c253e88a77b0189583201fb551b86aec7221ce7c3e079810",
            },
            "a95e11fa07f7b196ef488e73f67afbbbc16cf6a2c6de5f8d54ea49821fe604e6",
            "f9d5b5e3eb7657cf1bbba4cc856651864df9cd9fd9a6be9b9bc5fcbb67150deb",
            "005bfaeb3fd3d734e5c7020857fa18927064bf014679ec309bd3385b9a28f3ca",
        ),
        "n-body": (
            "1000",
            "50000000",
            {
                "nomo": "30fb086f8d5c55e0b389b7451f921a463c953db286ec9f97bb55d8b7bc595988",
                "c": "a8649dd7babc5b9178fc363f4d61b468662c703668c2f8f4ddeab206b3e7e879",
                "cpp": "1c0b0942c7075dbfcfa3a2b08ee486f284325e17525b2c2f86301c0ec0a2b492",
                "semantic-c": "284d2282e34a4a43c64dddc034cc7d087d7f1c62ed5c3097a7321d94f3123cd5",
                "go": "83e645802266f9d30a1093e97fc934dbb7dc6bd55ada49f58f45d63340c6e76a",
            },
            "76de83d6d51a74f82828547423f516e7815cfe8cfea6290a2831aa173f806de7",
            "3e6c9ef9d26cfe312a4cd8e1b81b3f671b88fbce84de543e8c23c206a942504d",
            "0cd1b9fd7f3ae069fce5a33ab6e92ea10e72844cb77f23d4947fd51e589e313c",
        ),
        "fannkuch-redux": (
            "7",
            "12",
            {
                "nomo": "0f6d0156c03cc3218b06a1adf560c5a3e3a99188fe9b7185b619b8a4ad8881e9",
                "c": "4d3135b2ed7a2fedb12b731c0f1a6bf901d763ac8421208fab6c4997c3ca9d80",
                "cpp": "28db75a4483148d430a2312dfd9088c46edf21b661c062d7ca90caf27209d962",
                "semantic-c": "35141667a9a8ac43cfcf1d47f98e401646901a108c41dceec4da35f5891b4669",
                "go": "806403dc4801db2c7c8894f923d18bb3f0b8ed3741ac48890a65ccd38fcaa4a2",
            },
            "2dc0a3cd4a547ba69389f97f3b447bd4d487fe6216c3cacd2f9bf8c908dc127f",
            "4265a65135c506a68d90d6474003fb9030b7ee244a06c046bd89b3932a28ce20",
            "73cf8d9df45479fb59dce5ac34102dafba368aa4aa7cb4655512bbd1dc5ca00e",
        ),
    }
    for workload in workloads:
        workload_id = str(workload["id"])
        (
            correctness_input,
            performance_input,
            source_shas,
            correctness_sha,
            formal_sha,
            project_manifest_sha,
        ) = frozen[workload_id]
        if workload.get("correctness_input") != correctness_input:
            raise HarnessError(f"{workload_id} correctness input changed")
        if workload.get("performance_input") != performance_input:
            raise HarnessError(f"{workload_id} formal input changed")
        sources = workload.get("sources", {})
        if set(sources) != {"nomo", "c", "cpp", "semantic-c", "go"}:
            raise HarnessError(f"{workload_id} v2 sources are incomplete")
        if {name: source.get("sha256") for name, source in sources.items()} != source_shas:
            raise HarnessError(f"{workload_id} frozen source SHA set changed")
        mappings = sources["cpp"].get("allocation_mappings")
        if (
            not isinstance(mappings, list)
            or canonical_json_sha256(mappings)
            != EXPECTED_ALLOCATION_MAPPING_SHA256[workload_id]
            or any(
                mapping.get("no_growth_or_reallocation") is not True
                or mapping.get("custom_allocator") is not False
                for mapping in mappings
            )
        ):
            raise HarnessError(
                f"{workload_id} C++ allocation mappings changed"
            )
        if workload.get("fixtures", {}).get("correctness", {}).get(
            "sha256"
        ) != correctness_sha:
            raise HarnessError(f"{workload_id} correctness fixture SHA changed")
        if workload.get("fixtures", {}).get("performance", {}).get(
            "sha256"
        ) != formal_sha:
            raise HarnessError(f"{workload_id} formal fixture SHA changed")
        for source_name, source in sources.items():
            source_path = resolve_suite_path(
                suite_root,
                str(source.get("path", "")),
                f"{workload_id} {source_name} source",
            )
            if source.get("sha256") != v1.sha256_file(source_path):
                raise HarnessError(
                    f"{workload_id} {source_name} source SHA-256 mismatch"
                )
        nomo_manifest = resolve_suite_path(
            suite_root,
            str(sources["nomo"].get("project_manifest", "")),
            f"{workload_id} Nomo project manifest",
        )
        if (
            sources["nomo"].get("project_manifest_sha256")
            != project_manifest_sha
            or project_manifest_sha != v1.sha256_file(nomo_manifest)
        ):
            raise HarnessError(f"{workload_id} Nomo project manifest changed")
        for fixture_name in ("correctness", "performance"):
            fixture = workload["fixtures"][fixture_name]
            fixture_path = resolve_suite_path(
                suite_root,
                str(fixture.get("path", "")),
                f"{workload_id} {fixture_name} fixture",
            )
            if fixture.get("sha256") != v1.sha256_file(fixture_path):
                raise HarnessError(
                    f"{workload_id} {fixture_name} fixture SHA-256 mismatch"
                )
            if b"\r\n" in fixture_path.read_bytes():
                raise HarnessError(
                    f"{workload_id} {fixture_name} fixture must use canonical LF"
                )
        if sources["semantic-c"].get("decisional") is not False:
            raise HarnessError("semantic-C must remain non-decisional")


def williams_schedule(
    lanes: Sequence[str] = TIMED_LANES, blocks: int = 30
) -> list[list[str]]:
    if len(lanes) != 5:
        raise HarnessError("the frozen Williams design requires five lanes")
    if blocks != 30:
        raise HarnessError("the frozen Williams design requires 30 blocks")
    base = [0, 1, 4, 2, 3]
    cycle = []
    for shift in range(5):
        row = [lanes[(index + shift) % 5] for index in base]
        cycle.append(row)
    cycle.extend([list(reversed(row)) for row in cycle[:5]])
    return [list(cycle[index % 10]) for index in range(blocks)]


def validate_williams_schedule(
    schedule: Sequence[Sequence[str]], lanes: Sequence[str] = TIMED_LANES
) -> None:
    if len(schedule) != 30:
        raise HarnessError("Williams schedule must contain 30 blocks")
    position_counts = {
        lane: [0 for _ in lanes]
        for lane in lanes
    }
    adjacency_counts = {
        (left, right): 0
        for left in lanes
        for right in lanes
        if left != right
    }
    for row in schedule:
        if len(row) != len(lanes) or set(row) != set(lanes):
            raise HarnessError("every Williams row must contain every lane once")
        for position, lane in enumerate(row):
            position_counts[lane][position] += 1
        for left, right in zip(row, row[1:]):
            adjacency_counts[(left, right)] += 1
    if any(count != 6 for counts in position_counts.values() for count in counts):
        raise HarnessError("Williams schedule is not position-balanced")
    if any(count != 6 for count in adjacency_counts.values()):
        raise HarnessError("Williams schedule is not carryover-balanced")


def paired_log_statistics(
    candidate_wall_ns: Sequence[int], comparator_wall_ns: Sequence[int]
) -> Dict[str, Any]:
    if len(candidate_wall_ns) != 30 or len(comparator_wall_ns) != 30:
        raise HarnessError("paired log statistics require exactly 30 blocks")
    logs = []
    for candidate, comparator in zip(candidate_wall_ns, comparator_wall_ns):
        if candidate <= 0 or comparator <= 0:
            raise HarnessError("paired wall times must be positive")
        logs.append(math.log(candidate / comparator))
    mean_log = statistics.fmean(logs)
    standard_deviation = statistics.stdev(logs)
    standard_error = standard_deviation / math.sqrt(30)
    upper_log = mean_log + T_CRITICAL_99_DF29 * standard_error
    return {
        "sample_count": 30,
        "degrees_of_freedom": 29,
        "confidence_level": 0.99,
        "confidence_side": "upper-one-sided",
        "distribution": "student-t",
        "t_critical": T_CRITICAL_99_DF29,
        "log_ratios": logs,
        "mean_log_ratio": mean_log,
        "sample_standard_deviation": standard_deviation,
        "standard_error": standard_error,
        "point_ratio": math.exp(mean_log),
        "upper_bound_99": math.exp(upper_log),
        "interpretation": "A ratio below 1.0 means candidate Nomo was faster.",
    }


def suite_log_statistics(
    workload_log_ratios: Sequence[Sequence[float]],
) -> Dict[str, Any]:
    if len(workload_log_ratios) != 3:
        raise HarnessError("suite statistics require exactly three workloads")
    if any(len(values) != 30 for values in workload_log_ratios):
        raise HarnessError("suite statistics require 30 paired blocks")
    suite_logs = [
        statistics.fmean(values[block] for values in workload_log_ratios)
        for block in range(30)
    ]
    mean_log = statistics.fmean(suite_logs)
    standard_deviation = statistics.stdev(suite_logs)
    standard_error = standard_deviation / math.sqrt(30)
    return {
        "sample_count": 30,
        "degrees_of_freedom": 29,
        "confidence_level": 0.99,
        "confidence_side": "upper-one-sided",
        "distribution": "student-t",
        "t_critical": T_CRITICAL_99_DF29,
        "suite_block_log_ratios": suite_logs,
        "mean_log_ratio": mean_log,
        "sample_standard_deviation": standard_deviation,
        "standard_error": standard_error,
        "point_ratio": math.exp(mean_log),
        "upper_bound_99": math.exp(
            mean_log + T_CRITICAL_99_DF29 * standard_error
        ),
        "weighting": "equal workload weight within each paired block",
        "interpretation": "A ratio below 1.0 means candidate Nomo was faster.",
    }


def evaluate_batch(
    workload_samples: Sequence[Dict[str, Any]],
    thresholds: Dict[str, Any],
    environment_eligible: bool,
) -> Dict[str, Any]:
    if len(workload_samples) != 3:
        raise HarnessError("a batch must contain all three workloads")
    workload_results = []
    logs_by_comparator = {
        comparator: []
        for comparator in (*DECISIVE_COMPARATORS, *DIAGNOSTIC_COMPARATORS)
    }
    workload_limits = thresholds["workload"]
    for workload in workload_samples:
        samples = workload.get("samples", {})
        if set(samples) != set(TIMED_LANES):
            raise HarnessError(f"{workload.get('id')} timed lanes are incomplete")
        candidate = [int(item["wall_ns"]) for item in samples["candidate"]]
        comparisons = {}
        for comparator in (*DECISIVE_COMPARATORS, *DIAGNOSTIC_COMPARATORS):
            comparator_values = [
                int(item["wall_ns"]) for item in samples[comparator]
            ]
            comparison = paired_log_statistics(candidate, comparator_values)
            comparisons[comparator] = comparison
            logs_by_comparator[comparator].append(comparison["log_ratios"])
        gates = {
            "candidate_vs_c": {
                "threshold": workload_limits["c_u99_max"],
                "actual": comparisons["c"]["upper_bound_99"],
                "passed": comparisons["c"]["upper_bound_99"]
                <= workload_limits["c_u99_max"],
            },
            "candidate_vs_cpp": {
                "threshold": workload_limits["cpp_u99_max"],
                "actual": comparisons["cpp"]["upper_bound_99"],
                "passed": comparisons["cpp"]["upper_bound_99"]
                <= workload_limits["cpp_u99_max"],
            },
            "candidate_vs_main": {
                "threshold": workload_limits["main_u99_max"],
                "actual": comparisons["main"]["upper_bound_99"],
                "passed": comparisons["main"]["upper_bound_99"]
                <= workload_limits["main_u99_max"],
            },
        }
        workload_results.append(
            {
                "id": workload["id"],
                "comparisons": comparisons,
                "gates": gates,
                "verdict": (
                    "pass" if all(gate["passed"] for gate in gates.values()) else "fail"
                ),
            }
        )

    suite_comparisons = {
        comparator: suite_log_statistics(logs_by_comparator[comparator])
        for comparator in (*DECISIVE_COMPARATORS, *DIAGNOSTIC_COMPARATORS)
    }
    suite_limits = thresholds["suite"]
    suite_gates = {
        "candidate_vs_c_point": {
            "threshold": suite_limits["c_point_max"],
            "actual": suite_comparisons["c"]["point_ratio"],
            "passed": suite_comparisons["c"]["point_ratio"]
            <= suite_limits["c_point_max"],
        },
        "candidate_vs_c_u99": {
            "threshold": suite_limits["c_u99_max"],
            "actual": suite_comparisons["c"]["upper_bound_99"],
            "passed": suite_comparisons["c"]["upper_bound_99"]
            <= suite_limits["c_u99_max"],
        },
        "candidate_vs_cpp_point": {
            "threshold": suite_limits["cpp_point_max"],
            "actual": suite_comparisons["cpp"]["point_ratio"],
            "passed": suite_comparisons["cpp"]["point_ratio"]
            <= suite_limits["cpp_point_max"],
        },
        "candidate_vs_cpp_u99": {
            "threshold": suite_limits["cpp_u99_max"],
            "actual": suite_comparisons["cpp"]["upper_bound_99"],
            "passed": suite_comparisons["cpp"]["upper_bound_99"]
            <= suite_limits["cpp_u99_max"],
        },
        "candidate_vs_main_u99": {
            "threshold": suite_limits["main_u99_max"],
            "actual": suite_comparisons["main"]["upper_bound_99"],
            "passed": suite_comparisons["main"]["upper_bound_99"]
            <= suite_limits["main_u99_max"],
        },
    }
    all_workloads_pass = all(
        item["verdict"] == "pass" for item in workload_results
    )
    all_suite_gates_pass = all(gate["passed"] for gate in suite_gates.values())
    if not environment_eligible:
        verdict = "ineligible"
    else:
        verdict = "pass" if all_workloads_pass and all_suite_gates_pass else "fail"
    return {
        "workloads": workload_results,
        "suite": {
            "comparisons": suite_comparisons,
            "gates": suite_gates,
            "verdict": "pass" if all_suite_gates_pass else "fail",
        },
        "environment_eligible": environment_eligible,
        "verdict": verdict,
    }


def geometric_mean(values: Sequence[float]) -> float:
    if not values or any(value <= 0 for value in values):
        raise HarnessError("geometric mean requires positive values")
    return math.exp(statistics.fmean(math.log(value) for value in values))


def batch_stability(
    workload_samples: Sequence[Dict[str, Any]], contract: Dict[str, Any]
) -> Dict[str, Any]:
    drift_limit = float(contract["reference_drift_max"])
    rsd_limit = float(contract["paired_ratio_rsd_max"])
    reference_metrics = []
    diagnostic_reference_metrics = []
    paired_ratio_metrics = []
    issues = []
    warnings = []
    for workload in workload_samples:
        samples = workload.get("samples", {})
        candidate = [float(sample["wall_ns"]) for sample in samples["candidate"]]
        for lane in (
            *contract["decisive_reference_lanes"],
            *contract["diagnostic_reference_lanes"],
        ):
            walls = [float(sample["wall_ns"]) for sample in samples[lane]]
            if len(walls) != 30:
                raise HarnessError("reference drift requires 30 blocks")
            first = geometric_mean(walls[:15])
            second = geometric_mean(walls[15:])
            ratio = second / first
            drift = max(ratio, 1.0 / ratio) - 1.0
            metric = {
                "workload": workload["id"],
                "lane": lane,
                "first_half_geomean_wall_ns": first,
                "second_half_geomean_wall_ns": second,
                "symmetric_drift": drift,
                "threshold": drift_limit,
                "passed": drift <= drift_limit
                or math.isclose(drift, drift_limit, rel_tol=0.0, abs_tol=1e-12),
            }
            if lane in contract["decisive_reference_lanes"]:
                reference_metrics.append(metric)
                if not metric["passed"]:
                    issues.append(
                        f"{workload['id']} {lane} reference drift {drift:.6f} "
                        f"exceeds {drift_limit:.6f}"
                    )
            else:
                diagnostic_reference_metrics.append(metric)
                if not metric["passed"]:
                    warnings.append(
                        f"{workload['id']} {lane} diagnostic drift {drift:.6f} "
                        f"exceeds {drift_limit:.6f}; diagnostic only"
                    )
        for comparator in contract["paired_ratio_comparators"]:
            comparator_walls = [
                float(sample["wall_ns"]) for sample in samples[comparator]
            ]
            if len(candidate) != 30 or len(comparator_walls) != 30:
                raise HarnessError("paired-ratio RSD requires 30 blocks")
            ratios = [
                candidate_wall / comparator_wall
                for candidate_wall, comparator_wall in zip(
                    candidate, comparator_walls
                )
            ]
            mean_ratio = statistics.fmean(ratios)
            rsd = statistics.stdev(ratios) / mean_ratio
            metric = {
                "workload": workload["id"],
                "comparator": comparator,
                "mean_paired_ratio": mean_ratio,
                "sample_standard_deviation": statistics.stdev(ratios),
                "relative_standard_deviation": rsd,
                "threshold": rsd_limit,
                "passed": rsd <= rsd_limit
                or math.isclose(rsd, rsd_limit, rel_tol=0.0, abs_tol=1e-12),
            }
            paired_ratio_metrics.append(metric)
            if not metric["passed"]:
                issues.append(
                    f"{workload['id']} candidate/{comparator} paired-ratio "
                    f"RSD {rsd:.6f} exceeds {rsd_limit:.6f}"
                )
    return {
        "reference_drift": reference_metrics,
        "diagnostic_reference_drift": diagnostic_reference_metrics,
        "paired_ratio_rsd": paired_ratio_metrics,
        "issues": issues,
        "warnings": warnings,
        "valid": not issues,
        "outliers_removed": False,
    }


def collector_descriptor_for_host(host_os: str) -> Dict[str, Any]:
    common = {
        "host_os": host_os,
        "implementation_version": 1,
        "wall_clock": "time.perf_counter_ns",
        "wall_time_unit": "nanoseconds",
        "cpu_time_unit": "nanoseconds",
        "peak_rss_unit": "bytes",
        "stdout_normalization": STDOUT_NORMALIZATION,
    }
    if host_os == "Windows":
        return {
            **common,
            "id": "windows-job-object-v1",
            "cpu_times": "GetProcessTimes kernel+user",
            "peak_rss": "GetProcessMemoryInfo PeakWorkingSetSize",
            "timeout_scope": "Windows Job Object",
        }
    if host_os in {"Darwin", "Linux"}:
        return {
            **common,
            "id": "posix-wait4-v1",
            "cpu_times": "wait4 rusage user+system",
            "peak_rss": "wait4 rusage ru_maxrss normalized to bytes",
            "timeout_scope": "POSIX process group",
        }
    raise HarnessError(f"unsupported collector host OS: {host_os}")


class ProcessCollector:
    collector_id = "abstract"

    def descriptor(self) -> Dict[str, Any]:
        raise NotImplementedError

    def run(
        self,
        command: Sequence[str],
        expected_stdout: bytes,
        timeout_seconds: float,
        environment_overrides: Optional[Dict[str, str]] = None,
    ) -> Tuple[Dict[str, Any], bytes]:
        raise NotImplementedError


def _environment(
    overrides: Optional[Dict[str, str]],
) -> Tuple[Dict[str, str], Dict[str, str]]:
    requested = dict(overrides or {})
    if requested not in ({}, {"GOMAXPROCS": "1"}):
        raise HarnessError(
            "runtime environment overrides are limited to GOMAXPROCS=1"
        )
    environment = {"LC_ALL": "C", "LANG": "C"}
    if os.name == "nt":
        system_root = windows_system_directory().parent
        runtime_temp = system_root / "Temp"
        if not runtime_temp.is_dir():
            raise HarnessError(
                "canonical Windows system temp directory is missing"
            )
        environment.update(
            {
                "SystemRoot": str(system_root),
                "WINDIR": str(system_root),
                "TEMP": str(runtime_temp.resolve()),
                "TMP": str(runtime_temp.resolve()),
            }
        )
    else:
        runtime_temp = next(
            (
                path
                for path in (Path("/var/tmp"), Path("/tmp"))
                if path.is_dir()
            ),
            None,
        )
        if runtime_temp is None:
            raise HarnessError("canonical POSIX runtime temp directory is missing")
        environment.update(
            {
                "PATH": stable_build_path(),
                "TMPDIR": str(runtime_temp.resolve()),
            }
        )
    environment.update(requested)
    return dict(environment), dict(environment)


def validate_runtime_environment_authority(
    runtime_environments: Any,
    build_environment: Dict[str, Any],
    host_os: str,
) -> Dict[str, Dict[str, str]]:
    if (
        not isinstance(runtime_environments, dict)
        or set(runtime_environments) != {"default", "go"}
        or not all(
            isinstance(runtime_environments[name], dict)
            for name in ("default", "go")
        )
    ):
        raise HarnessError("runtime environment authority is incomplete")
    default = runtime_environments["default"]
    go = runtime_environments["go"]
    if go != {**default, "GOMAXPROCS": "1"}:
        raise HarnessError("Go runtime environment authority changed")
    if default.get("LC_ALL") != "C" or default.get("LANG") != "C":
        raise HarnessError("runtime locale authority changed")
    retained = build_environment.get("retained", {})
    if not isinstance(retained, dict):
        raise HarnessError("build environment retained authority is missing")
    if host_os == "Windows":
        expected_keys = {
            "LC_ALL",
            "LANG",
            "SystemRoot",
            "WINDIR",
            "TEMP",
            "TMP",
        }
        system_root = default.get("SystemRoot")
        if (
            set(default) != expected_keys
            or default.get("WINDIR") != system_root
            or system_root != retained.get("SystemRoot")
            or system_root != retained.get("WINDIR")
            or not artifact_path_is_absolute(system_root, host_os)
            or default.get("TEMP") != default.get("TMP")
            or artifact_path(default.get("TEMP"), host_os)
            != artifact_path(system_root, host_os) / "Temp"
        ):
            raise HarnessError("Windows runtime environment authority changed")
    elif host_os in {"Darwin", "Linux"}:
        if (
            set(default) != {"LC_ALL", "LANG", "PATH", "TMPDIR"}
            or default.get("PATH") != retained.get("PATH")
            or default.get("TMPDIR")
            not in {"/var/tmp", "/private/var/tmp", "/tmp", "/private/tmp"}
        ):
            raise HarnessError("POSIX runtime environment authority changed")
        entries = str(default["PATH"]).split(":")
        if (
            not entries
            or len(entries) != len(set(entries))
            or any(
                not PurePosixPath(entry).is_absolute()
                or "/.codex/tmp/arg0/" in f"{entry.replace(chr(92), '/')}/"
                for entry in entries
            )
        ):
            raise HarnessError("POSIX runtime PATH authority changed")
    else:
        raise HarnessError("runtime environment host OS is unsupported")
    return runtime_environments


def _validate_process_output(
    command: Sequence[str], exit_code: int, stdout: bytes, stderr: bytes, expected: bytes
) -> bytes:
    if exit_code != 0:
        raise HarnessError(
            f"benchmark exited {exit_code}: {v1.command_text(command)}\n"
            f"stderr:\n{stderr.decode('utf-8', errors='replace')}"
        )
    if stderr:
        raise HarnessError(
            f"benchmark wrote unexpected stderr: {v1.command_text(command)}\n"
            + stderr.decode("utf-8", errors="replace")
        )
    if b"\r\n" in expected:
        raise HarnessError("fixtures must use canonical LF line endings")
    normalized = stdout.replace(b"\r\n", b"\n")
    if normalized != expected:
        raise HarnessError(
            f"output mismatch for {v1.command_text(command)}: "
            f"expected {expected!r}, found raw={stdout!r}, normalized={normalized!r}"
        )
    return normalized


def _failure_sample(
    command: Sequence[str],
    environment: Dict[str, str],
    collector_id: str,
    started_at_utc: str,
    wall_ns: int,
    stdout: bytes,
    stderr: bytes,
    *,
    exit_code: Optional[int],
    timed_out: bool,
    failure_kind: str,
    message: str,
    user_cpu_ns: Optional[int] = None,
    system_cpu_ns: Optional[int] = None,
    peak_rss_bytes: Optional[int] = None,
    failure_source: str = "process-collector",
) -> Dict[str, Any]:
    normalized = stdout.replace(b"\r\n", b"\n")
    return {
        "status": "failed",
        "failure_kind": failure_kind,
        "failure_message": message,
        "failure_source": failure_source,
        "started_at_utc": started_at_utc,
        "finished_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "command_argv": [str(part) for part in command],
        "command": v1.command_text(command),
        "environment": environment,
        "collector": collector_id,
        "stdout": "captured-failed",
        "wall_ns": max(1, wall_ns),
        "user_cpu_ns": user_cpu_ns,
        "system_cpu_ns": system_cpu_ns,
        "cpu_total_ns": (
            user_cpu_ns + system_cpu_ns
            if user_cpu_ns is not None and system_cpu_ns is not None
            else None
        ),
        "peak_rss_bytes": peak_rss_bytes,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "stdout_raw_sha256": v1.sha256_bytes(stdout),
        "stdout_normalized_sha256": v1.sha256_bytes(normalized),
        "stdout_raw_base64": base64.b64encode(stdout).decode("ascii"),
        "stdout_normalization": STDOUT_NORMALIZATION,
        "stdout_bytes": {"raw": len(stdout), "normalized": len(normalized)},
        "stderr": {
            "sha256": v1.sha256_bytes(stderr),
            "length_bytes": len(stderr),
            "preview_utf8": stderr[:4096].decode("utf-8", errors="replace"),
        },
    }


class WindowsProcessMemoryCounters(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


class WindowsIoCounters(ctypes.Structure):
    _fields_ = [
        ("ReadOperationCount", ctypes.c_ulonglong),
        ("WriteOperationCount", ctypes.c_ulonglong),
        ("OtherOperationCount", ctypes.c_ulonglong),
        ("ReadTransferCount", ctypes.c_ulonglong),
        ("WriteTransferCount", ctypes.c_ulonglong),
        ("OtherTransferCount", ctypes.c_ulonglong),
    ]


class WindowsJobObjectBasicLimitInformation(ctypes.Structure):
    _fields_ = [
        ("PerProcessUserTimeLimit", ctypes.c_longlong),
        ("PerJobUserTimeLimit", ctypes.c_longlong),
        ("LimitFlags", wintypes.DWORD),
        ("MinimumWorkingSetSize", ctypes.c_size_t),
        ("MaximumWorkingSetSize", ctypes.c_size_t),
        ("ActiveProcessLimit", wintypes.DWORD),
        ("Affinity", ctypes.c_size_t),
        ("PriorityClass", wintypes.DWORD),
        ("SchedulingClass", wintypes.DWORD),
    ]


class WindowsJobObjectExtendedLimitInformation(ctypes.Structure):
    _fields_ = [
        ("BasicLimitInformation", WindowsJobObjectBasicLimitInformation),
        ("IoInfo", WindowsIoCounters),
        ("ProcessMemoryLimit", ctypes.c_size_t),
        ("JobMemoryLimit", ctypes.c_size_t),
        ("PeakProcessMemoryUsed", ctypes.c_size_t),
        ("PeakJobMemoryUsed", ctypes.c_size_t),
    ]


class WindowsStartupInfo(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("lpReserved", wintypes.LPWSTR),
        ("lpDesktop", wintypes.LPWSTR),
        ("lpTitle", wintypes.LPWSTR),
        ("dwX", wintypes.DWORD),
        ("dwY", wintypes.DWORD),
        ("dwXSize", wintypes.DWORD),
        ("dwYSize", wintypes.DWORD),
        ("dwXCountChars", wintypes.DWORD),
        ("dwYCountChars", wintypes.DWORD),
        ("dwFillAttribute", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("wShowWindow", wintypes.WORD),
        ("cbReserved2", wintypes.WORD),
        ("lpReserved2", ctypes.POINTER(ctypes.c_ubyte)),
        ("hStdInput", wintypes.HANDLE),
        ("hStdOutput", wintypes.HANDLE),
        ("hStdError", wintypes.HANDLE),
    ]


class WindowsProcessInformation(ctypes.Structure):
    _fields_ = [
        ("hProcess", wintypes.HANDLE),
        ("hThread", wintypes.HANDLE),
        ("dwProcessId", wintypes.DWORD),
        ("dwThreadId", wintypes.DWORD),
    ]


class WindowsStartupInfoEx(ctypes.Structure):
    _fields_ = [
        ("StartupInfo", WindowsStartupInfo),
        ("lpAttributeList", ctypes.c_void_p),
    ]


def configure_windows_api(kernel32: Any, psapi: Any) -> Dict[str, Any]:
    """Attach pointer-width-safe Win32 signatures and return auditable metadata."""
    kernel32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.SetInformationJobObject.argtypes = [
        wintypes.HANDLE,
        ctypes.c_int,
        ctypes.c_void_p,
        wintypes.DWORD,
    ]
    kernel32.SetInformationJobObject.restype = wintypes.BOOL
    kernel32.InitializeProcThreadAttributeList.argtypes = [
        ctypes.c_void_p,
        wintypes.DWORD,
        wintypes.DWORD,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    kernel32.InitializeProcThreadAttributeList.restype = wintypes.BOOL
    kernel32.UpdateProcThreadAttribute.argtypes = [
        ctypes.c_void_p,
        wintypes.DWORD,
        ctypes.c_size_t,
        ctypes.c_void_p,
        ctypes.c_size_t,
        ctypes.c_void_p,
        ctypes.c_void_p,
    ]
    kernel32.UpdateProcThreadAttribute.restype = wintypes.BOOL
    kernel32.DeleteProcThreadAttributeList.argtypes = [ctypes.c_void_p]
    kernel32.DeleteProcThreadAttributeList.restype = None
    kernel32.CreateProcessW.argtypes = [
        wintypes.LPCWSTR,
        wintypes.LPWSTR,
        ctypes.c_void_p,
        ctypes.c_void_p,
        wintypes.BOOL,
        wintypes.DWORD,
        ctypes.c_void_p,
        wintypes.LPCWSTR,
        ctypes.c_void_p,
        ctypes.POINTER(WindowsProcessInformation),
    ]
    kernel32.CreateProcessW.restype = wintypes.BOOL
    kernel32.ResumeThread.argtypes = [wintypes.HANDLE]
    kernel32.ResumeThread.restype = wintypes.DWORD
    kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel32.WaitForSingleObject.restype = wintypes.DWORD
    kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel32.TerminateJobObject.restype = wintypes.BOOL
    kernel32.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel32.TerminateProcess.restype = wintypes.BOOL
    kernel32.GetExitCodeProcess.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(wintypes.DWORD),
    ]
    kernel32.GetExitCodeProcess.restype = wintypes.BOOL
    kernel32.GetProcessTimes.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(wintypes.FILETIME),
        ctypes.POINTER(wintypes.FILETIME),
        ctypes.POINTER(wintypes.FILETIME),
        ctypes.POINTER(wintypes.FILETIME),
    ]
    kernel32.GetProcessTimes.restype = wintypes.BOOL
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    psapi.GetProcessMemoryInfo.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(WindowsProcessMemoryCounters),
        wintypes.DWORD,
    ]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    return {
        "pointer_width_bits": ctypes.sizeof(ctypes.c_void_p) * 8,
        "handle_width_bits": ctypes.sizeof(wintypes.HANDLE) * 8,
        "create_job_restype": "HANDLE",
        "process_handle_arguments": "HANDLE",
        "launch_order": (
            "create-job,initialize-job-list-attribute,"
            "create-suspended-atomically-in-job,start-timer,resume-thread"
        ),
        "atomic_job_association": "PROC_THREAD_ATTRIBUTE_JOB_LIST",
        "kill_on_job_close": True,
    }


def initialize_windows_job_list_attribute(
    kernel32: Any,
    job: Any,
) -> Tuple[Any, ctypes.c_void_p, Any]:
    attribute_bytes = ctypes.c_size_t()
    kernel32.InitializeProcThreadAttributeList(
        None, 1, 0, ctypes.byref(attribute_bytes)
    )
    if attribute_bytes.value == 0:
        raise HarnessError(
            "InitializeProcThreadAttributeList size query failed"
        )
    storage = ctypes.create_string_buffer(attribute_bytes.value)
    attribute_list = ctypes.cast(storage, ctypes.c_void_p)
    if not kernel32.InitializeProcThreadAttributeList(
        attribute_list,
        1,
        0,
        ctypes.byref(attribute_bytes),
    ):
        raise HarnessError("InitializeProcThreadAttributeList failed")
    job_handles = (wintypes.HANDLE * 1)(job)
    proc_thread_attribute_job_list = 0x0002000D
    if not kernel32.UpdateProcThreadAttribute(
        attribute_list,
        0,
        proc_thread_attribute_job_list,
        ctypes.cast(job_handles, ctypes.c_void_p),
        ctypes.sizeof(job_handles),
        None,
        None,
    ):
        kernel32.DeleteProcThreadAttributeList(attribute_list)
        raise HarnessError(
            "UpdateProcThreadAttribute(PROC_THREAD_ATTRIBUTE_JOB_LIST) failed"
        )
    return storage, attribute_list, job_handles


class PosixWait4Collector(ProcessCollector):
    collector_id = "posix-wait4-v1"

    def descriptor(self) -> Dict[str, Any]:
        return collector_descriptor_for_host(platform.system())

    def run(
        self,
        command: Sequence[str],
        expected_stdout: bytes,
        timeout_seconds: float,
        environment_overrides: Optional[Dict[str, str]] = None,
    ) -> Tuple[Dict[str, Any], bytes]:
        environment, recorded_environment = _environment(environment_overrides)
        with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
            started_at_utc = dt.datetime.now(dt.timezone.utc).isoformat()
            process = subprocess.Popen(
                [str(part) for part in command],
                stdin=subprocess.DEVNULL,
                stdout=stdout_file,
                stderr=stderr_file,
                env=environment,
                start_new_session=True,
            )
            started = time.perf_counter_ns()
            deadline = time.monotonic() + timeout_seconds
            usage = None
            status = None
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
                    stdout_file.seek(0)
                    stderr_file.seek(0)
                    stdout = stdout_file.read()
                    stderr = stderr_file.read()
                    message = (
                        f"command exceeded {timeout_seconds:.3f}s and was killed: "
                        f"{v1.command_text(command)}"
                    )
                    record = _failure_sample(
                        command,
                        recorded_environment,
                        self.collector_id,
                        started_at_utc,
                        time.perf_counter_ns() - started,
                        stdout,
                        stderr,
                        exit_code=process.returncode,
                        timed_out=True,
                        failure_kind="timeout",
                        message=message,
                        user_cpu_ns=int(
                            round(waited_usage.ru_utime * 1_000_000_000)
                        ),
                        system_cpu_ns=int(
                            round(waited_usage.ru_stime * 1_000_000_000)
                        ),
                        peak_rss_bytes=v1.peak_rss_bytes(
                            waited_usage.ru_maxrss
                        ),
                    )
                    raise SampleTimeoutError(message, record)
                time.sleep(0.005)
            wall_ns = time.perf_counter_ns() - started
            assert usage is not None
            assert status is not None
            exit_code = os.waitstatus_to_exitcode(status)
            process.returncode = exit_code
            stdout_file.seek(0)
            stderr_file.seek(0)
            stdout = stdout_file.read()
            stderr = stderr_file.read()
        sample = {
            "started_at_utc": started_at_utc,
            "finished_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "command_argv": [str(part) for part in command],
            "command": v1.command_text(command),
            "environment": recorded_environment,
            "collector": self.collector_id,
            "stdout": "captured-and-verified",
            "wall_ns": wall_ns,
            "user_cpu_ns": int(round(usage.ru_utime * 1_000_000_000)),
            "system_cpu_ns": int(round(usage.ru_stime * 1_000_000_000)),
            "cpu_total_ns": int(
                round(usage.ru_utime * 1_000_000_000)
            )
            + int(
                round(usage.ru_stime * 1_000_000_000)
            ),
            "peak_rss_bytes": v1.peak_rss_bytes(usage.ru_maxrss),
            "exit_code": exit_code,
            "stdout_raw_sha256": v1.sha256_bytes(stdout),
            "stdout_normalized_sha256": v1.sha256_bytes(
                stdout.replace(b"\r\n", b"\n")
            ),
            "stdout_raw_base64": base64.b64encode(stdout).decode("ascii"),
            "stdout_normalization": STDOUT_NORMALIZATION,
        }
        try:
            normalized_stdout = _validate_process_output(
                command, exit_code, stdout, stderr, expected_stdout
            )
        except HarnessError as error:
            record = _failure_sample(
                command,
                recorded_environment,
                self.collector_id,
                started_at_utc,
                wall_ns,
                stdout,
                stderr,
                exit_code=exit_code,
                timed_out=False,
                failure_kind=(
                    "process-failure"
                    if exit_code != 0 or stderr
                    else "output-mismatch"
                ),
                message=str(error),
                user_cpu_ns=sample["user_cpu_ns"],
                system_cpu_ns=sample["system_cpu_ns"],
                peak_rss_bytes=sample["peak_rss_bytes"],
            )
            raise SampleCollectionError(str(error), record) from error
        return sample, normalized_stdout


class WindowsJobObjectCollector(ProcessCollector):
    collector_id = "windows-job-object-v1"

    def descriptor(self) -> Dict[str, Any]:
        return collector_descriptor_for_host("Windows")

    def run(
        self,
        command: Sequence[str],
        expected_stdout: bytes,
        timeout_seconds: float,
        environment_overrides: Optional[Dict[str, str]] = None,
    ) -> Tuple[Dict[str, Any], bytes]:
        if os.name != "nt":
            raise HarnessError("Windows Job Object collector is available only on Windows")

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        api_metadata = configure_windows_api(kernel32, psapi)
        if api_metadata["pointer_width_bits"] != api_metadata["handle_width_bits"]:
            raise HarnessError("Win32 HANDLE width does not match process pointer width")

        environment, recorded_environment = _environment(environment_overrides)
        import msvcrt

        with (
            tempfile.TemporaryFile() as stdin_file,
            tempfile.TemporaryFile() as stdout_file,
            tempfile.TemporaryFile() as stderr_file,
        ):
            file_handles = [
                msvcrt.get_osfhandle(stream.fileno())
                for stream in (stdin_file, stdout_file, stderr_file)
            ]
            for file_handle in file_handles:
                os.set_handle_inheritable(file_handle, True)
            startup = WindowsStartupInfoEx()
            startup.StartupInfo.cb = ctypes.sizeof(startup)
            startup.StartupInfo.dwFlags = 0x00000100  # STARTF_USESTDHANDLES
            startup.StartupInfo.hStdInput = wintypes.HANDLE(file_handles[0])
            startup.StartupInfo.hStdOutput = wintypes.HANDLE(file_handles[1])
            startup.StartupInfo.hStdError = wintypes.HANDLE(file_handles[2])
            process_info = WindowsProcessInformation()
            started_at_utc = dt.datetime.now(dt.timezone.utc).isoformat()
            failure_started_ns = time.perf_counter_ns()
            job = kernel32.CreateJobObjectW(None, None)
            if not job:
                raise HarnessError("CreateJobObjectW failed")
            process_handle: Optional[int] = None
            thread_handle: Optional[int] = None
            assigned_to_job = False
            attribute_storage = None
            attribute_list = None
            job_handles = None
            try:
                limits = WindowsJobObjectExtendedLimitInformation()
                limits.BasicLimitInformation.LimitFlags = 0x00002000
                if not kernel32.SetInformationJobObject(
                    job, 9, ctypes.byref(limits), ctypes.sizeof(limits)
                ):
                    raise HarnessError("SetInformationJobObject failed")
                (
                    attribute_storage,
                    attribute_list,
                    job_handles,
                ) = initialize_windows_job_list_attribute(kernel32, job)
                startup.lpAttributeList = attribute_list
                command_line = ctypes.create_unicode_buffer(
                    subprocess.list2cmdline([str(part) for part in command])
                )
                environment_block = ctypes.create_unicode_buffer(
                    "\0".join(
                        f"{key}={value}"
                        for key, value in sorted(
                            environment.items(), key=lambda item: item[0].upper()
                        )
                    )
                    + "\0\0"
                )
                creation_flags = (
                    0x00000004  # CREATE_SUSPENDED
                    | 0x00000200  # CREATE_NEW_PROCESS_GROUP
                    | 0x00000400  # CREATE_UNICODE_ENVIRONMENT
                    | 0x00080000  # EXTENDED_STARTUPINFO_PRESENT
                )
                if not kernel32.CreateProcessW(
                    None,
                    command_line,
                    None,
                    None,
                    True,
                    creation_flags,
                    environment_block,
                    None,
                    ctypes.byref(startup),
                    ctypes.byref(process_info),
                ):
                    raise HarnessError("CreateProcessW(CREATE_SUSPENDED) failed")
                process_handle = int(process_info.hProcess)
                thread_handle = int(process_info.hThread)
                assigned_to_job = True
                started = time.perf_counter_ns()
                if kernel32.ResumeThread(process_info.hThread) == 0xFFFFFFFF:
                    raise HarnessError("ResumeThread failed")
                wait_result = kernel32.WaitForSingleObject(
                    process_info.hProcess,
                    max(1, int(math.ceil(timeout_seconds * 1000))),
                )
                if wait_result == 0x00000102:
                    terminated = bool(kernel32.TerminateJobObject(job, 1))
                    cleanup_wait = kernel32.WaitForSingleObject(
                        process_info.hProcess, 5000
                    )
                    stdout_file.seek(0)
                    stderr_file.seek(0)
                    stdout = stdout_file.read()
                    stderr = stderr_file.read()
                    message = (
                        f"command exceeded {timeout_seconds:.3f}s and its Job Object "
                        f"was terminated: {v1.command_text(command)}; "
                        f"cleanup_terminate={terminated}; "
                        f"cleanup_wait_result={cleanup_wait}"
                    )
                    raise SampleTimeoutError(
                        message,
                        _failure_sample(
                            command,
                            recorded_environment,
                            self.collector_id,
                            started_at_utc,
                            time.perf_counter_ns() - started,
                            stdout,
                            stderr,
                            exit_code=None,
                            timed_out=True,
                            failure_kind="timeout",
                            message=message,
                        ),
                    )
                if wait_result != 0:
                    kernel32.TerminateJobObject(job, 1)
                    raise HarnessError("WaitForSingleObject failed")
                wall_ns = time.perf_counter_ns() - started
                creation = wintypes.FILETIME()
                exit_time = wintypes.FILETIME()
                kernel_time = wintypes.FILETIME()
                user_time = wintypes.FILETIME()
                if not kernel32.GetProcessTimes(
                    process_info.hProcess,
                    ctypes.byref(creation),
                    ctypes.byref(exit_time),
                    ctypes.byref(kernel_time),
                    ctypes.byref(user_time),
                ):
                    raise HarnessError("GetProcessTimes failed")
                memory = WindowsProcessMemoryCounters()
                memory.cb = ctypes.sizeof(memory)
                if not psapi.GetProcessMemoryInfo(
                    process_info.hProcess,
                    ctypes.byref(memory),
                    ctypes.sizeof(memory),
                ):
                    raise HarnessError("GetProcessMemoryInfo failed")
                exit_code_value = wintypes.DWORD()
                if not kernel32.GetExitCodeProcess(
                    process_info.hProcess, ctypes.byref(exit_code_value)
                ):
                    raise HarnessError("GetExitCodeProcess failed")
                exit_code = int(exit_code_value.value)
            except SampleCollectionError:
                raise
            except HarnessError as error:
                cleanup_events = []
                if process_handle is not None:
                    if assigned_to_job:
                        terminated_job = bool(
                            kernel32.TerminateJobObject(job, 1)
                        )
                        cleanup_events.append(
                            {
                                "api": "TerminateJobObject",
                                "succeeded": terminated_job,
                                "last_error": (
                                    None
                                    if terminated_job
                                    else getattr(
                                        ctypes,
                                        "get_last_error",
                                        lambda: None,
                                    )()
                                ),
                            }
                        )
                    terminated_process = bool(
                        kernel32.TerminateProcess(
                            process_info.hProcess, 2
                        )
                    )
                    cleanup_wait = kernel32.WaitForSingleObject(
                        process_info.hProcess, 5000
                    )
                    cleanup_events.append(
                        {
                            "api": "TerminateProcess+WaitForSingleObject",
                            "succeeded": cleanup_wait == 0,
                            "terminate_succeeded": terminated_process,
                            "wait_result": cleanup_wait,
                            "last_error": (
                                None
                                if terminated_process
                                else getattr(
                                    ctypes,
                                    "get_last_error",
                                    lambda: None,
                                )()
                            ),
                        }
                    )
                stdout_file.seek(0)
                stderr_file.seek(0)
                stdout = stdout_file.read()
                stderr = stderr_file.read()
                message = (
                    f"Windows collector API failure: {error}; "
                    f"cleanup_events={cleanup_events}"
                )
                raise SampleCollectionError(
                    message,
                    _failure_sample(
                        command,
                        recorded_environment,
                        self.collector_id,
                        started_at_utc,
                        time.perf_counter_ns() - failure_started_ns,
                        stdout,
                        stderr,
                        exit_code=None,
                        timed_out=False,
                        failure_kind="collector-failure",
                        message=message,
                        failure_source="windows-job-object-api",
                    ),
                ) from error
            finally:
                if attribute_list is not None:
                    kernel32.DeleteProcThreadAttributeList(attribute_list)
                if thread_handle is not None:
                    kernel32.CloseHandle(wintypes.HANDLE(thread_handle))
                if process_handle is not None:
                    kernel32.CloseHandle(wintypes.HANDLE(process_handle))
                kernel32.CloseHandle(job)
                for file_handle in file_handles:
                    os.set_handle_inheritable(file_handle, False)
            stdout_file.seek(0)
            stderr_file.seek(0)
            stdout = stdout_file.read()
            stderr = stderr_file.read()

        def filetime_100ns(value: Any) -> int:
            return (int(value.dwHighDateTime) << 32) | int(value.dwLowDateTime)

        user_ns = filetime_100ns(user_time) * 100
        system_ns = filetime_100ns(kernel_time) * 100
        sample = {
            "started_at_utc": started_at_utc,
            "finished_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "command_argv": [str(part) for part in command],
            "command": v1.command_text(command),
            "environment": recorded_environment,
            "collector": self.collector_id,
            "stdout": "captured-and-verified",
            "wall_ns": wall_ns,
            "user_cpu_ns": user_ns,
            "system_cpu_ns": system_ns,
            "cpu_total_ns": user_ns + system_ns,
            "peak_rss_bytes": int(memory.PeakWorkingSetSize),
            "exit_code": exit_code,
            "stdout_raw_sha256": v1.sha256_bytes(stdout),
            "stdout_normalized_sha256": v1.sha256_bytes(
                stdout.replace(b"\r\n", b"\n")
            ),
            "stdout_raw_base64": base64.b64encode(stdout).decode("ascii"),
            "stdout_normalization": STDOUT_NORMALIZATION,
        }
        try:
            normalized_stdout = _validate_process_output(
                command, exit_code, stdout, stderr, expected_stdout
            )
        except HarnessError as error:
            raise SampleCollectionError(
                str(error),
                _failure_sample(
                    command,
                    recorded_environment,
                    self.collector_id,
                    started_at_utc,
                    wall_ns,
                    stdout,
                    stderr,
                    exit_code=exit_code,
                    timed_out=False,
                    failure_kind=(
                        "process-failure"
                        if exit_code != 0 or stderr
                        else "output-mismatch"
                    ),
                    message=str(error),
                    user_cpu_ns=user_ns,
                    system_cpu_ns=system_ns,
                    peak_rss_bytes=int(memory.PeakWorkingSetSize),
                ),
            ) from error
        return sample, normalized_stdout


def select_collector() -> ProcessCollector:
    if os.name == "nt":
        return WindowsJobObjectCollector()
    if hasattr(os, "wait4"):
        return PosixWait4Collector()
    raise HarnessError("no supported per-process resource collector for this platform")


def parse_clang_version(output: str) -> str:
    match = re.search(r"(?:Apple )?clang version ([^\s]+)", output, re.IGNORECASE)
    if match is None:
        raise ToolchainMismatch("the configured compiler is not Clang")
    return match.group(1)


def parse_clang_installation(output: str, executable: Path) -> str:
    match = re.search(r"^InstalledDir:\s*(.+)$", output, re.MULTILINE)
    return str(Path(match.group(1)).resolve()) if match else str(executable.parent)


def build_probe_output(
    executable: Path, arguments: Sequence[str]
) -> Tuple[str, Dict[str, Any]]:
    record, stdout, stderr = run_build_capture(
        [str(executable), *arguments],
        30.0,
    )
    output = (stdout + stderr).decode("utf-8", errors="replace").strip()
    if not output:
        raise HarnessError(f"empty probe output from {executable}")
    return output, record


def clang_selected_driver(
    executable: Path, driver_name: str
) -> Dict[str, Any]:
    invocation_path = Path(os.path.abspath(executable))
    invocation_realpath = invocation_path.resolve()
    selection_command = None
    selected_path = invocation_realpath
    if platform.system() == "Darwin":
        xcrun = Path("/usr/bin/xcrun")
        if not xcrun.is_file():
            raise ToolchainMismatch("Darwin Clang authority requires /usr/bin/xcrun")
        selection_command, stdout, _ = run_build_capture(
            [str(xcrun), "--find", driver_name],
            30.0,
        )
        selected_text = stdout.decode("utf-8", errors="strict").strip()
        if not selected_text or not Path(selected_text).is_absolute():
            raise ToolchainMismatch(
                f"xcrun did not select an absolute {driver_name} path"
            )
        selected_path = Path(selected_text)
        if not selected_path.is_file():
            raise ToolchainMismatch(
                f"xcrun selected unavailable {driver_name}: {selected_path}"
            )
        if (
            invocation_path.parent != Path("/usr/bin")
            and invocation_realpath != selected_path.resolve()
        ):
            raise ToolchainMismatch(
                f"configured {driver_name} does not match xcrun selection"
            )
    selected_realpath = selected_path.resolve()
    return {
        "path": str(invocation_path),
        "invocation_realpath": str(invocation_realpath),
        "invocation_sha256": v1.sha256_file(invocation_realpath),
        "selected_path": str(selected_path),
        "realpath": str(selected_realpath),
        "sha256": v1.sha256_file(selected_realpath),
        "selection_command": selection_command,
    }


def clang_target(executable: Path) -> Tuple[str, Dict[str, Any]]:
    record, stdout, stderr = run_build_capture(
        [
            str(executable),
            *CLANG_DRIVER_CONFIG_FLAGS,
            "-print-target-triple",
        ],
        30.0,
    )
    target = (stdout + stderr).decode("utf-8", errors="replace").strip()
    if not target or any(character.isspace() for character in target):
        raise ToolchainMismatch(
            f"{executable} did not provide one machine-readable target triple"
        )
    return target, record


def inspect_toolchains(
    manifest: Dict[str, Any],
    nomo_argument: str,
    clang_argument: str,
    clangxx_argument: str,
    go_argument: str,
) -> Dict[str, Any]:
    nomo = resolve_executable(nomo_argument, "Nomo")
    clang = resolve_executable(clang_argument, "Clang C")
    clangxx = resolve_executable(clangxx_argument, "Clang C++")
    go = resolve_executable(go_argument, "Go")
    nomo_help, nomo_version_command = build_probe_output(nomo, ["--help"])
    nomo_version = v1.parse_nomo_version(nomo_help)
    clang_identity = clang_selected_driver(clang, "clang")
    clangxx_identity = clang_selected_driver(clangxx, "clang++")
    clang_output, clang_version_command = build_probe_output(
        clang, [*CLANG_DRIVER_CONFIG_FLAGS, "--version"]
    )
    clangxx_output, clangxx_version_command = build_probe_output(
        clangxx, [*CLANG_DRIVER_CONFIG_FLAGS, "--version"]
    )
    c_version = parse_clang_version(clang_output)
    cpp_version = parse_clang_version(clangxx_output)
    c_installation = parse_clang_installation(clang_output, clang)
    cpp_installation = parse_clang_installation(clangxx_output, clangxx)
    c_target, c_target_command = clang_target(clang)
    cpp_target, cpp_target_command = clang_target(clangxx)
    go_output, go_version_command = build_probe_output(go, ["version"])
    go_fields = go_output.split()
    go_version = go_fields[2] if len(go_fields) >= 3 else ""
    goroot_output, goroot_command = build_probe_output(go, ["env", "GOROOT"])
    goroot = goroot_output.strip()
    if not goroot or not Path(goroot).is_absolute() or not Path(goroot).is_dir():
        raise ToolchainMismatch(
            f"Go returned an invalid GOROOT for {go}: {goroot!r}"
        )
    mismatches = []
    expected_nomo = manifest["toolchains"]["correctness_baseline_nomo"][
        "required_version"
    ]
    expected_go = manifest["toolchains"]["go"]["required_version"]
    if nomo_version != expected_nomo:
        mismatches.append(f"Nomo expected {expected_nomo}, found {nomo_version}")
    if go_version != expected_go:
        mismatches.append(f"Go expected {expected_go}, found {go_version}")
    if os.name == "nt":
        llvm_tools = canonical_windows_build_support()["authority"][
            "llvm_tools"
        ]
        for driver_name, identity, authority_name in (
            ("clang", clang_identity, "clang.exe"),
            ("clang++", clangxx_identity, "clang++.exe"),
        ):
            authority = llvm_tools[authority_name]
            if (
                identity["path"] != authority["path"]
                or identity["realpath"] != authority["realpath"]
                or identity["sha256"] != authority["sha256"]
            ):
                mismatches.append(
                    f"{driver_name} does not match the authority-bound "
                    "Windows LLVM driver"
                )
    if c_version != cpp_version:
        mismatches.append(
            f"Clang C/C++ version mismatch: C {c_version}, C++ {cpp_version}"
        )
    if c_installation != cpp_installation:
        mismatches.append(
            "Clang C/C++ installation mismatch: "
            f"C {c_installation}, C++ {cpp_installation}"
        )
    if c_target != cpp_target:
        mismatches.append(
            f"Clang C/C++ target mismatch: C {c_target}, C++ {cpp_target}"
        )
    if clang == clangxx:
        mismatches.append(
            "Clang C/C++ driver invocation paths must be distinct; "
            "clang++ mode may not be replaced with clang"
        )
    if mismatches:
        raise ToolchainMismatch("toolchain mismatch: " + "; ".join(mismatches))
    return {
        "build_environment": sanitized_build_environment()[1],
        "runtime_environments": {
            "default": _environment({})[1],
            "go": _environment({"GOMAXPROCS": "1"})[1],
        },
        "nomo": {
            "path": str(nomo),
            "realpath": str(nomo.resolve()),
            "version": nomo_version,
            "version_output": nomo_help.splitlines()[0],
            "sha256": v1.sha256_file(nomo),
            "version_command": nomo_version_command,
        },
        "clang": {
            **clang_identity,
            "version": c_version,
            "version_output": clang_output,
            "version_command": clang_version_command,
            "installation": c_installation,
            "target_triple": c_target,
            "target_command": c_target_command,
            "driver_config_flags": list(CLANG_DRIVER_CONFIG_FLAGS),
        },
        "clangxx": {
            **clangxx_identity,
            "version": cpp_version,
            "version_output": clangxx_output,
            "version_command": clangxx_version_command,
            "installation": cpp_installation,
            "target_triple": cpp_target,
            "target_command": cpp_target_command,
            "driver_config_flags": list(CLANG_DRIVER_CONFIG_FLAGS),
        },
        "go": {
            "path": str(go),
            "realpath": str(go.resolve()),
            "sha256": v1.sha256_file(go),
            "version": go_version,
            "version_output": go_output,
            "version_command": go_version_command,
            "goroot": goroot,
            "goroot_command": goroot_command,
        },
    }


def release_capability(nomo: Path, label: str) -> Dict[str, Any]:
    try:
        record, stdout, stderr = run_build_capture(
            [str(nomo), "build", "--help"], 30.0
        )
    except HarnessError as error:
        return {
            "label": label,
            "status": "unavailable",
            "reason": f"cannot inspect Nomo help: {error}",
            "emit_c_fallback_used": False,
        }
    text = (stdout + stderr).decode("utf-8", errors="replace")
    available = "--release" in text
    return {
        "label": label,
        "status": "available" if available else "unavailable",
        "reason": (
            "public nomo build --release is present"
            if available
            else "public nomo build --release is absent; fallback is forbidden"
        ),
        "help_command": record,
        "nomo_path": str(nomo),
        "nomo_sha256": v1.sha256_file(nomo),
        "emit_c_fallback_used": False,
    }


def emit_c_capability(nomo: Path, label: str) -> Dict[str, Any]:
    try:
        record, stdout, stderr = run_build_capture(
            [str(nomo), "build", "--help"], 30.0
        )
    except HarnessError as error:
        return {
            "label": label,
            "status": "unavailable",
            "reason": f"cannot inspect Nomo help: {error}",
        }
    text = (stdout + stderr).decode("utf-8", errors="replace")
    available = "--emit-c" in text
    return {
        "label": label,
        "status": "available" if available else "unavailable",
        "reason": (
            "public nomo build --emit-c is present"
            if available
            else "public nomo build --emit-c is absent"
        ),
        "help_command": record,
        "nomo_path": str(nomo),
        "nomo_sha256": v1.sha256_file(nomo),
    }


def memory_bytes() -> Optional[int]:
    if sys.platform == "darwin":
        text = v1.system_capture(["sysctl", "-n", "hw.memsize"])
        return int(text) if text and text.isdigit() else None
    if os.name == "posix" and Path("/proc/meminfo").is_file():
        for line in Path("/proc/meminfo").read_text(
            encoding="utf-8", errors="replace"
        ).splitlines():
            if line.startswith("MemTotal:"):
                fields = line.split()
                if len(fields) >= 2 and fields[1].isdigit():
                    return int(fields[1]) * 1024
    if os.name == "nt":
        class MemoryStatus(ctypes.Structure):
            _fields_ = [
                ("dwLength", ctypes.c_ulong),
                ("dwMemoryLoad", ctypes.c_ulong),
                ("ullTotalPhys", ctypes.c_ulonglong),
                ("ullAvailPhys", ctypes.c_ulonglong),
                ("ullTotalPageFile", ctypes.c_ulonglong),
                ("ullAvailPageFile", ctypes.c_ulonglong),
                ("ullTotalVirtual", ctypes.c_ulonglong),
                ("ullAvailVirtual", ctypes.c_ulonglong),
                ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
            ]
        status = MemoryStatus()
        status.dwLength = ctypes.sizeof(status)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
            return int(status.ullTotalPhys)
    return None


def host_provenance() -> Dict[str, Any]:
    host = v1.host_provenance()
    clock = time.get_clock_info("perf_counter")
    host.update(
        {
            "kernel": platform.release(),
            "memory_bytes": memory_bytes(),
            "clock": {
                "implementation": clock.implementation,
                "monotonic": clock.monotonic,
                "adjustable": clock.adjustable,
                "resolution_seconds": clock.resolution,
            },
            "python": platform.python_version(),
        }
    )
    return host


def _raw_text_evidence(text: str) -> Dict[str, Any]:
    encoded = text.encode("utf-8")
    return {
        "sha256": v1.sha256_bytes(encoded),
        "length_bytes": len(encoded),
        "text": text,
    }


def _raw_json_evidence(value: Any) -> Dict[str, Any]:
    return _raw_text_evidence(
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        )
    )


def dynamic_command_environment() -> Dict[str, str]:
    return {
        "LC_ALL": "C",
        "LANG": "C",
        "PATH": stable_build_path(),
    }


def resolve_dynamic_executable(value: str) -> Path:
    candidate = Path(value)
    if candidate.is_absolute():
        if not candidate.is_file():
            raise HarnessError(
                f"dynamic environment executable is missing: {value}"
            )
        return candidate.resolve()
    resolved = (
        _windows_executable_from_path(value, stable_build_path())
        if os.name == "nt"
        else shutil.which(value, path=stable_build_path())
    )
    if resolved is None:
        raise HarnessError(
            f"dynamic environment executable is not on the controlled PATH: {value}"
        )
    return Path(resolved).resolve()


def windows_system_directory(kernel32: Optional[Any] = None) -> Path:
    if kernel32 is None:
        if os.name != "nt":
            raise HarnessError(
                "GetSystemDirectoryW is available only on Windows"
            )
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetSystemDirectoryW.argtypes = [
        wintypes.LPWSTR,
        wintypes.UINT,
    ]
    kernel32.GetSystemDirectoryW.restype = wintypes.UINT
    buffer = ctypes.create_unicode_buffer(32768)
    length = int(kernel32.GetSystemDirectoryW(buffer, len(buffer)))
    if length == 0 or length >= len(buffer):
        raise HarnessError("GetSystemDirectoryW failed")
    directory = Path(buffer.value)
    if not directory.is_absolute():
        raise HarnessError("GetSystemDirectoryW returned a non-absolute path")
    return directory.resolve()


def windows_temp_directory(kernel32: Optional[Any] = None) -> Path:
    if kernel32 is None:
        if os.name != "nt":
            raise HarnessError("GetTempPathW is available only on Windows")
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetTempPathW.argtypes = [wintypes.DWORD, wintypes.LPWSTR]
    kernel32.GetTempPathW.restype = wintypes.DWORD
    buffer = ctypes.create_unicode_buffer(32768)
    length = int(kernel32.GetTempPathW(len(buffer), buffer))
    if length == 0 or length >= len(buffer):
        raise HarnessError("GetTempPathW failed")
    directory = Path(buffer.value)
    if not directory.is_absolute() or not directory.is_dir():
        raise HarnessError("GetTempPathW returned an unavailable path")
    return directory.resolve()


def _stable_command_record(
    record: Dict[str, Any], environment: Dict[str, str]
) -> Dict[str, Any]:
    return {
        "argv": record["argv"],
        "command": record["command"],
        "cwd": record["cwd"],
        "exit_code": record["exit_code"],
        "environment": dict(sorted(environment.items())),
    }


def _windows_tool_identity(path: Path) -> Dict[str, str]:
    invocation = Path(os.path.abspath(path))
    realpath = invocation.resolve()
    if not realpath.is_file():
        raise HarnessError(f"Windows tool is unavailable: {invocation}")
    return {
        "path": str(invocation),
        "realpath": str(realpath),
        "sha256": v1.sha256_file(realpath),
    }


def canonical_darwin_build_support(
    *, refresh: bool = False
) -> Dict[str, Any]:
    global _DARWIN_BUILD_SUPPORT_CACHE
    if platform.system() != "Darwin":
        raise HarnessError(
            "canonical Darwin SDK support is available only on macOS"
        )
    if _DARWIN_BUILD_SUPPORT_CACHE is not None and not refresh:
        return _DARWIN_BUILD_SUPPORT_CACHE
    xcrun = Path("/usr/bin/xcrun")
    if not xcrun.is_file():
        raise HarnessError("trusted /usr/bin/xcrun is unavailable")
    environment = {
        "PATH": stable_build_path(),
        "LC_ALL": "C",
        "LANG": "C",
    }
    if value := os.environ.get("TMPDIR"):
        environment["TMPDIR"] = str(Path(value).resolve())
    command = [str(xcrun), "--sdk", "macosx", "--show-sdk-path"]
    record, stdout, stderr = v1.run_capture(
        command,
        30.0,
        environment=environment,
    )
    if stderr:
        raise HarnessError("xcrun SDK selection wrote unexpected stderr")
    sdkroot = Path(stdout.decode("utf-8", errors="strict").strip())
    approved_roots = (Path("/Applications"), Path("/Library/Developer"))
    if (
        not sdkroot.is_absolute()
        or not sdkroot.is_dir()
        or not any(
            sdkroot.resolve().is_relative_to(root.resolve())
            for root in approved_roots
            if root.is_dir()
        )
    ):
        raise HarnessError(f"xcrun selected an untrusted macOS SDK: {sdkroot}")
    settings_candidates = (
        sdkroot / "SDKSettings.plist",
        sdkroot / "SDKSettings.json",
    )
    settings = next(
        (path for path in settings_candidates if path.is_file()), None
    )
    if settings is None:
        raise HarnessError("selected macOS SDK lacks SDKSettings authority")
    support = {
        "schema": 1,
        "xcrun": _windows_tool_identity(xcrun),
        "selection_command": _stable_command_record(record, environment),
        "sdkroot": str(sdkroot.resolve()),
        "sdk_settings": {
            "path": str(settings.resolve()),
            "sha256": v1.sha256_file(settings),
        },
    }
    _DARWIN_BUILD_SUPPORT_CACHE = support
    return support


def windows_known_folder(folder_id: str) -> Path:
    if os.name != "nt":
        raise HarnessError("Known Folder discovery is available only on Windows")

    class GUID(ctypes.Structure):
        _fields_ = [
            ("data1", wintypes.DWORD),
            ("data2", wintypes.WORD),
            ("data3", wintypes.WORD),
            ("data4", ctypes.c_ubyte * 8),
        ]

    shell32 = ctypes.WinDLL("shell32", use_last_error=True)
    ole32 = ctypes.WinDLL("ole32", use_last_error=True)
    shell32.SHGetKnownFolderPath.argtypes = [
        ctypes.POINTER(GUID),
        wintypes.DWORD,
        wintypes.HANDLE,
        ctypes.POINTER(ctypes.c_wchar_p),
    ]
    shell32.SHGetKnownFolderPath.restype = ctypes.c_long
    ole32.CoTaskMemFree.argtypes = [ctypes.c_void_p]
    ole32.CoTaskMemFree.restype = None
    guid = GUID.from_buffer_copy(uuid.UUID(folder_id).bytes_le)
    value = ctypes.c_wchar_p()
    result = int(
        shell32.SHGetKnownFolderPath(
            ctypes.byref(guid), 0, None, ctypes.byref(value)
        )
    )
    try:
        if result != 0 or not value.value:
            raise HarnessError(
                f"SHGetKnownFolderPath failed for {folder_id}: HRESULT {result}"
            )
        path = Path(value.value)
        if not path.is_absolute() or not path.is_dir():
            raise HarnessError(
                f"Known Folder path is unavailable for {folder_id}: {path}"
            )
        return path.resolve()
    finally:
        if value:
            ole32.CoTaskMemFree(ctypes.cast(value, ctypes.c_void_p))


def canonical_windows_seed_environment(
    system_directory: Path, temp_directory: Path
) -> Dict[str, str]:
    program_data = windows_known_folder(
        "62ab5d82-fdc1-4dc3-a9dd-070d1d495d97"
    )
    program_files = windows_known_folder(
        "905e63b6-c1bf-494e-b29c-65b732d3d21a"
    )
    program_files_x86 = windows_known_folder(
        "7c5a40ef-a0fb-4bfc-874a-c0f2e0b9fa8e"
    )
    program_files_x64 = windows_known_folder(
        "6d809377-6af0-444b-8957-a3773f02200e"
    )
    system_root = system_directory.parent
    cmd = system_directory / "cmd.exe"
    if not cmd.is_file():
        raise HarnessError(f"trusted COMSPEC is unavailable: {cmd}")
    machine = platform.machine().lower()
    architecture = "ARM64" if machine in {"arm64", "aarch64"} else "AMD64"
    return {
        "SystemRoot": str(system_root),
        "WINDIR": str(system_root),
        "TEMP": str(temp_directory),
        "TMP": str(temp_directory),
        "ProgramData": str(program_data),
        "ProgramFiles": str(program_files),
        "ProgramFiles(x86)": str(program_files_x86),
        "ProgramW6432": str(program_files_x64),
        "COMSPEC": str(cmd),
        "PATHEXT": ";".join(WINDOWS_CANONICAL_PATHEXT),
        "PROCESSOR_ARCHITECTURE": architecture,
        "PATH": str(system_directory),
    }


def _visual_studio_install_date(value: Any) -> dt.datetime:
    if not isinstance(value, str) or not value:
        raise ValueError("missing installDate")
    parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None or parsed.utcoffset() != dt.timedelta(0):
        raise ValueError("installDate is not UTC")
    return parsed


def select_visual_studio_installation(
    installations: Any,
) -> Tuple[Dict[str, Any], List[Dict[str, Any]], str]:
    if not isinstance(installations, list):
        raise HarnessError("vswhere did not return an installation array")
    candidates = []
    eligible = []
    for raw in installations:
        if not isinstance(raw, dict):
            raise HarnessError("vswhere returned a non-object installation")
        raw_path = str(raw.get("installationPath", ""))
        version = str(raw.get("installationVersion", ""))
        product_id = str(raw.get("productId", ""))
        catalog = raw.get("catalog")
        product_line = (
            str(catalog.get("productLine", ""))
            if isinstance(catalog, dict)
            else ""
        )
        install_date = raw.get("installDate")
        version_parts = version.split(".")
        reasons = []
        path = Path(raw_path)
        if not path.is_absolute() or not path.is_dir():
            reasons.append("installation path is unavailable")
        if not version_parts or any(not part.isdigit() for part in version_parts):
            reasons.append("installation version is not numeric")
        if raw.get("isComplete") is not True:
            reasons.append("installation is not complete")
        if raw.get("isLaunchable") is not True:
            reasons.append("installation is not launchable")
        if raw.get("isPrerelease") is not False:
            reasons.append("installation is prerelease or unspecified")
        if product_line != "Dev17":
            reasons.append("installation productLine is not Visual Studio 2022")
        try:
            parsed_install_date = _visual_studio_install_date(install_date)
        except (TypeError, ValueError):
            parsed_install_date = None
            reasons.append("installation installDate is not canonical UTC")
        vsdevcmd = path / "Common7" / "Tools" / "VsDevCmd.bat"
        if not vsdevcmd.is_file():
            reasons.append("VsDevCmd.bat is unavailable")
        record = {
            "installation_path": (
                str(path.resolve()) if path.is_absolute() and path.exists() else raw_path
            ),
            "installation_version": version,
            "product_id": product_id,
            "product_line": product_line,
            "install_date": install_date,
            "is_complete": raw.get("isComplete"),
            "is_launchable": raw.get("isLaunchable"),
            "is_prerelease": raw.get("isPrerelease"),
            "eligible": not reasons,
            "reasons": reasons,
        }
        candidates.append(record)
        if not reasons:
            eligible.append(
                (
                    tuple(int(part) for part in version_parts),
                    parsed_install_date,
                    record["installation_path"].casefold(),
                    raw,
                )
            )
    if not eligible:
        raise HarnessError("vswhere found no complete Visual Studio with VC tools")
    newest_version = max(item[0] for item in eligible)
    newest = [item for item in eligible if item[0] == newest_version]
    newest_date = max(item[1] for item in newest)
    newest = [item for item in newest if item[1] == newest_date]
    normalized_paths = [item[2] for item in newest]
    if len(normalized_paths) != len(set(normalized_paths)):
        raise HarnessError(
            "vswhere returned ambiguous top-ranked Visual Studio records"
        )
    _, _, _, selected = min(newest, key=lambda item: item[2])
    selected_path = str(Path(str(selected["installationPath"])).resolve())
    reason = (
        "selected complete, launchable, non-prerelease Visual Studio 2022 by "
        "numeric installationVersion descending, installDate descending, "
        "then lexicographically smallest "
        f"canonical installation path ({selected_path})"
    )
    candidates.sort(
        key=lambda item: (
            item["installation_version"],
            item["installation_path"].casefold(),
        )
    )
    return selected, candidates, reason


def windows_vsdevcmd_command(
    cmd: Path, vsdevcmd: Path, architecture: str
) -> List[str]:
    return [
        str(cmd),
        "/d",
        "/c",
        "call",
        str(vsdevcmd),
        "-no_logo",
        f"-arch={architecture}",
        f"-host_arch={architecture}",
        "&&",
        "set",
    ]


def _raw_command_stream(content: bytes) -> Dict[str, Any]:
    return {
        "base64": base64.b64encode(content).decode("ascii"),
        "length_bytes": len(content),
        "sha256": v1.sha256_bytes(content),
    }


def _filter_paths_within_roots(
    values: Sequence[str],
    roots: Sequence[Path],
    label: str,
    *,
    require_nonempty: bool = True,
) -> Tuple[List[str], List[Dict[str, str]]]:
    canonical_roots = [root.resolve() for root in roots]
    canonical = []
    excluded = []
    for value in values:
        if not value:
            continue
        path = Path(value).resolve()
        if not path.is_dir():
            excluded.append(
                {"path": str(path), "reason": "directory is unavailable"}
            )
            continue
        if not any(
            path == root or path.is_relative_to(root) for root in canonical_roots
        ):
            excluded.append(
                {
                    "path": str(path),
                    "reason": "outside selected VS VC and Windows SDK/UCRT roots",
                }
            )
            continue
        if str(path) not in canonical:
            canonical.append(str(path))
    if require_nonempty and not canonical:
        raise HarnessError(f"{label} did not contain any canonical paths")
    return canonical, excluded


def _windows_authority_marker(
    directories: Sequence[str], filename: str, label: str
) -> Dict[str, str]:
    matches = sorted(
        {
            (Path(directory) / filename).resolve()
            for directory in directories
            if (Path(directory) / filename).is_file()
        },
        key=lambda path: str(path).casefold(),
    )
    if len(matches) != 1:
        raise HarnessError(
            f"Windows SDK/CRT authority requires exactly one {label} marker; "
            f"found {len(matches)}"
        )
    return {
        "path": str(matches[0]),
        "sha256": v1.sha256_file(matches[0]),
    }


def _windows_executable_from_path(
    name: str, path_value: str, *, require_unique: bool = False
) -> Optional[Path]:
    if Path(name).name != name:
        raise HarnessError(
            f"canonical Windows executable lookup requires a basename: {name}"
        )
    candidate_names = (
        [name]
        if Path(name).suffix
        else [
            f"{name}{extension}"
            for extension in WINDOWS_CANONICAL_PATHEXT
        ]
    )
    matches = []
    for directory in path_value.split(os.pathsep):
        if not directory:
            continue
        for candidate_name in candidate_names:
            candidate = (Path(directory) / candidate_name).resolve()
            if candidate.is_file() and candidate not in matches:
                matches.append(candidate)
    if require_unique and len(matches) != 1:
        raise HarnessError(
            f"canonical Windows PATH must select exactly one {name}; "
            f"found {len(matches)}"
        )
    return matches[0] if matches else None


def canonical_windows_build_support(
    *, refresh: bool = False
) -> Dict[str, Any]:
    global _WINDOWS_BUILD_SUPPORT_CACHE
    if os.name != "nt":
        raise HarnessError(
            "canonical Windows build support is available only on Windows"
        )
    if _WINDOWS_BUILD_SUPPORT_CACHE is not None and not refresh:
        return _WINDOWS_BUILD_SUPPORT_CACHE

    system_directory = windows_system_directory()
    system_root = system_directory.parent
    temp_directory = windows_temp_directory()
    minimal_environment = canonical_windows_seed_environment(
        system_directory, temp_directory
    )
    cmd = Path(minimal_environment["COMSPEC"])
    vswhere_candidates = tuple(
        Path(root)
        / "Microsoft Visual Studio"
        / "Installer"
        / "vswhere.exe"
        for root in dict.fromkeys(
            (
                minimal_environment["ProgramFiles(x86)"],
                minimal_environment["ProgramFiles"],
                minimal_environment["ProgramW6432"],
            )
        )
    )
    vswhere = next((path for path in vswhere_candidates if path.is_file()), None)
    if vswhere is None:
        raise HarnessError(
            "trusted Visual Studio Installer or cmd.exe is unavailable"
        )
    vswhere_argv = [
        str(vswhere),
        "-all",
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-format",
        "json",
        "-utf8",
    ]
    vswhere_record, vswhere_stdout, vswhere_stderr = v1.run_capture(
        vswhere_argv,
        30.0,
        environment=minimal_environment,
    )
    try:
        installations = json.loads(
            vswhere_stdout.decode("utf-8-sig", errors="strict")
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise HarnessError("vswhere did not return canonical JSON") from error
    installation, installation_candidates, selection_reason = (
        select_visual_studio_installation(installations)
    )
    installation_path = Path(str(installation.get("installationPath", "")))
    installation_version = str(installation.get("installationVersion", ""))
    if (
        not installation_path.is_absolute()
        or not installation_path.is_dir()
        or not installation_version
    ):
        raise HarnessError("vswhere Visual Studio selection is incomplete")
    vsdevcmd = installation_path / "Common7" / "Tools" / "VsDevCmd.bat"
    if not vsdevcmd.is_file():
        raise HarnessError(f"trusted VsDevCmd is unavailable: {vsdevcmd}")
    machine = platform.machine().lower()
    architecture = "arm64" if machine in {"arm64", "aarch64"} else "amd64"
    vsdevcmd_argv = windows_vsdevcmd_command(
        cmd, vsdevcmd, architecture
    )
    vsdevcmd_record, environment_stdout, _ = v1.run_capture(
        vsdevcmd_argv,
        60.0,
        environment=minimal_environment,
    )
    discovered = {}
    for raw_line in environment_stdout.decode(
        "utf-8", errors="replace"
    ).splitlines():
        key, separator, value = raw_line.partition("=")
        if separator:
            discovered[key.upper()] = value
    required = {"PATH", "INCLUDE", "LIB", "LIBPATH"}
    if not required.issubset(discovered) or any(
        not discovered[name] for name in required
    ):
        raise HarnessError("VsDevCmd did not publish complete compiler paths")
    selected_names = required | {
        "UCRTVERSION",
        "UNIVERSALCRTSDKDIR",
        "VCINSTALLDIR",
        "VCTOOLSINSTALLDIR",
        "VCTOOLSVERSION",
        "VSCMD_ARG_TGT_ARCH",
        "VSINSTALLDIR",
        "WINDOWSSDKDIR",
        "WINDOWSSDKVERSION",
    }
    environment = {
        name: discovered[name]
        for name in sorted(selected_names)
        if discovered.get(name)
    }
    environment.update(
        {
            name: value
            for name, value in minimal_environment.items()
            if name != "PATH"
        }
    )
    llvm_directories = sorted(
        {
            (Path(root) / "LLVM" / "bin").resolve()
            for root in (
                minimal_environment["ProgramFiles"],
                minimal_environment["ProgramW6432"],
                minimal_environment["ProgramFiles(x86)"],
            )
            if (Path(root) / "LLVM" / "bin" / "clang.exe").is_file()
            and (Path(root) / "LLVM" / "bin" / "clang++.exe").is_file()
        },
        key=lambda path: str(path).casefold(),
    )
    if len(llvm_directories) != 1:
        raise HarnessError(
            "trusted Windows LLVM discovery requires exactly one "
            "Program Files LLVM driver directory"
        )
    llvm_directory = llvm_directories[0]
    llvm_tool_identities = {
        name: _windows_tool_identity(llvm_directory / name)
        for name in ("clang.exe", "clang++.exe")
    }
    selected_root = installation_path.resolve()
    vs_path, excluded_path = _filter_paths_within_roots(
        environment["PATH"].split(os.pathsep),
        [selected_root],
        "PATH",
    )
    tool_identities = {}
    for name in ("cl.exe", "link.exe"):
        selected = _windows_executable_from_path(
            name, os.pathsep.join(vs_path), require_unique=True
        )
        assert selected is not None
        tool_identities[name] = _windows_tool_identity(selected)
    for name, identity in tool_identities.items():
        tool_path = Path(identity["realpath"])
        if not tool_path.is_relative_to(selected_root):
            raise HarnessError(f"{name} is outside selected Visual Studio")
    sdk_roots = []
    trusted_program_roots = [
        Path(minimal_environment[name]).resolve()
        for name in ("ProgramFiles", "ProgramFiles(x86)", "ProgramW6432")
    ]
    for name in ("WINDOWSSDKDIR", "UNIVERSALCRTSDKDIR"):
        value = environment.get(name)
        if value:
            path = Path(value).resolve()
            if not path.is_dir() or not any(
                path == root or path.is_relative_to(root)
                for root in trusted_program_roots
            ):
                raise HarnessError(f"{name} is unavailable: {path}")
            sdk_roots.append(path)
    if not sdk_roots or not environment.get("WINDOWSSDKVERSION"):
        raise HarnessError("VsDevCmd did not publish a complete Windows SDK")
    allowed_roots = [selected_root, *sdk_roots]
    include, excluded_include = _filter_paths_within_roots(
        environment["INCLUDE"].split(os.pathsep), allowed_roots, "INCLUDE"
    )
    lib, excluded_lib = _filter_paths_within_roots(
        environment["LIB"].split(os.pathsep), allowed_roots, "LIB"
    )
    libpath, excluded_libpath = _filter_paths_within_roots(
        environment["LIBPATH"].split(os.pathsep),
        allowed_roots,
        "LIBPATH",
        require_nonempty=False,
    )
    canonical_path = []
    for path in (
        llvm_directory,
        Path(tool_identities["cl.exe"]["realpath"]).parent,
        Path(tool_identities["link.exe"]["realpath"]).parent,
        system_directory,
    ):
        value = str(path.resolve())
        if value not in canonical_path:
            canonical_path.append(value)
    environment["PATH"] = os.pathsep.join(canonical_path)
    environment["INCLUDE"] = os.pathsep.join(include)
    environment["LIB"] = os.pathsep.join(lib)
    environment["LIBPATH"] = os.pathsep.join(libpath)
    sdk_crt_markers = {
        "ucrt_header": _windows_authority_marker(
            include, "ctype.h", "UCRT ctype.h"
        ),
        "windows_header": _windows_authority_marker(
            include, "windows.h", "Windows SDK windows.h"
        ),
        "sdk_version_header": _windows_authority_marker(
            include, "sdkddkver.h", "Windows SDK sdkddkver.h"
        ),
        "vc_runtime_header": _windows_authority_marker(
            include, "vcruntime.h", "VC runtime vcruntime.h"
        ),
        "ucrt_library": _windows_authority_marker(
            lib, "ucrt.lib", "UCRT ucrt.lib"
        ),
        "kernel32_library": _windows_authority_marker(
            lib, "kernel32.lib", "Windows SDK kernel32.lib"
        ),
        "vc_runtime_library": _windows_authority_marker(
            lib, "libcmt.lib", "VC runtime libcmt.lib"
        ),
    }
    authority = {
        "schema": 1,
        "architecture": architecture,
        "vswhere": _windows_tool_identity(vswhere),
        "vswhere_command": _stable_command_record(
            vswhere_record, minimal_environment
        ),
        "vswhere_stdout": _raw_command_stream(vswhere_stdout),
        "vswhere_stderr": _raw_command_stream(vswhere_stderr),
        "installation_path": str(installation_path.resolve()),
        "installation_version": installation_version,
        "installation_candidates": installation_candidates,
        "installation_selection_reason": selection_reason,
        "chosen_installation_json": installation,
        "vsdevcmd": _windows_tool_identity(vsdevcmd),
        "vsdevcmd_command": _stable_command_record(
            vsdevcmd_record, minimal_environment
        ),
        "windows_sdk_version": environment.get("WINDOWSSDKVERSION"),
        "vc_tools_version": environment.get("VCTOOLSVERSION"),
        "tools": tool_identities,
        "llvm_tools": llvm_tool_identities,
        "sdk_crt_markers": sdk_crt_markers,
        "excluded_candidates": {
            "path": excluded_path,
            "include": excluded_include,
            "lib": excluded_lib,
            "libpath": excluded_libpath,
        },
        "include": include,
        "lib": lib,
        "libpath": libpath,
        "path": environment["PATH"].split(os.pathsep),
    }
    support = {"environment": environment, "authority": authority}
    _WINDOWS_BUILD_SUPPORT_CACHE = support
    return support


def _dynamic_command(command: Sequence[str]) -> Dict[str, Any]:
    requested = [str(part) for part in command]
    environment = dynamic_command_environment()
    identity = None
    try:
        executable = resolve_dynamic_executable(requested[0])
        argv = [str(executable), *requested[1:]]
        identity = {
            "path": str(executable),
            "realpath": str(executable.resolve()),
            "sha256": v1.sha256_file(executable),
            "version_output": None,
        }
        completed = subprocess.run(
            argv,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
            env=environment,
        )
    except (HarnessError, OSError, subprocess.TimeoutExpired) as error:
        return {
            "status": "unavailable",
            "source": "command",
            "command_argv": (
                [identity["path"], *requested[1:]]
                if identity is not None
                else requested
            ),
            "command_identity": identity,
            "environment": environment,
            "reason": str(error),
            "raw": _raw_text_evidence(""),
            "parsed": None,
        }
    raw = completed.stdout + completed.stderr
    return {
        "status": "captured" if completed.returncode == 0 else "unavailable",
        "source": "command",
        "command_argv": argv,
        "command_identity": identity,
        "environment": environment,
        "exit_code": completed.returncode,
        "raw": _raw_text_evidence(raw.decode("utf-8", errors="replace")),
    }


def _parsed_observation(
    captured: Dict[str, Any],
    parsed: Any,
    qualified: bool,
    reason: str,
) -> Dict[str, Any]:
    return {
        **captured,
        "status": "qualified" if qualified else "failed",
        "parsed": parsed,
        "reason": reason,
    }


def _parse_byte_quantity(value: str, unit: str) -> int:
    scale = {
        "K": 1024,
        "M": 1024**2,
        "G": 1024**3,
        "T": 1024**4,
    }[unit.upper()]
    return int(round(float(value) * scale))


def parse_darwin_thermal(text: str) -> Dict[str, Any]:
    speed_match = re.search(r"CPU_Speed_Limit\s*=\s*(\d+)", text)
    scheduler_match = re.search(
        r"(?:CPU_)?Scheduler_Limit\s*=\s*(\d+)", text
    )
    available_match = re.search(
        r"CPU_Available_CPUs\s*=\s*(\d+)", text
    )
    lower_lines = [line.strip().lower() for line in text.splitlines()]
    thermal_warning = any(
        "thermal warning" in line
        and "no thermal warning level has been recorded" not in line
        for line in lower_lines
    )
    performance_warning = any(
        "performance warning" in line
        and "no performance warning level has been recorded" not in line
        for line in lower_lines
    )
    speed = int(speed_match.group(1)) if speed_match else None
    scheduler = (
        int(scheduler_match.group(1)) if scheduler_match else None
    )
    available_cpus = (
        int(available_match.group(1)) if available_match else None
    )
    nonempty_lines = tuple(
        line.strip() for line in text.splitlines() if line.strip()
    )
    no_recorded_complete = (
        nonempty_lines == DARWIN_PMSET_NO_RECORDED_LINES
    )
    recognized_complete = no_recorded_complete
    degradation_reasons = []
    if not recognized_complete:
        degradation_reasons.append(
            "pmset thermal output shape was incomplete or invalid"
        )
    if speed is not None and speed < 100:
        degradation_reasons.append("CPU_Speed_Limit below 100")
    if scheduler is not None and scheduler < 100:
        degradation_reasons.append("CPU_Scheduler_Limit below 100")
    if thermal_warning:
        degradation_reasons.append("thermal warning was published")
    if performance_warning:
        degradation_reasons.append("performance warning was published")
    return {
        "cpu_speed_limit_percent": speed,
        "scheduler_limit_percent": scheduler,
        "available_cpus": available_cpus,
        "shape": (
            "complete-no-recorded"
            if no_recorded_complete
            else "unrecognized"
        ),
        "recognized_complete": recognized_complete,
        "thermal_warning_published": thermal_warning,
        "performance_warning_published": performance_warning,
        "explicit_degradation": bool(degradation_reasons),
        "degradation_reasons": degradation_reasons,
    }


def parse_darwin_frequency_observation(text: str) -> Dict[str, Any]:
    return {
        "applicability": "not-applicable",
        "platform": "Darwin-Apple-Silicon",
        "governor_exposed": False,
        "auxiliary_pmset": parse_darwin_thermal(text),
    }


def parse_darwin_process_thermal_state(text: str) -> Dict[str, Any]:
    value_text = text.strip()
    value = int(value_text) if re.fullmatch(r"[0-3]", value_text) else None
    labels = {
        0: "nominal",
        1: "fair",
        2: "serious",
        3: "critical",
    }
    return {
        "thermal_state": value,
        "thermal_state_name": labels.get(value),
        "normal": value == 0,
        "api": "Foundation.NSProcessInfo.thermalState",
    }


def capture_dynamic_environment(
    authority_host_sha256: str,
    policy: Optional[Dict[str, Any]] = None,
    host_function: Callable[[], Dict[str, Any]] = host_provenance,
) -> Dict[str, Any]:
    active_policy = dict(policy or DYNAMIC_ENVIRONMENT_POLICY)
    if active_policy != DYNAMIC_ENVIRONMENT_POLICY:
        raise HarnessError("dynamic environment policy changed")
    captured_at = dt.datetime.now(dt.timezone.utc).isoformat()
    monotonic_ns = time.monotonic_ns()
    host = host_function()
    logical_cores = max(
        1,
        int(
            host.get("logical_core_count")
            or host.get("cpu_count_logical")
            or os.cpu_count()
            or 1
        ),
    )
    try:
        load_values = list(os.getloadavg())
        normalized_load = load_values[0] / logical_cores
        load_status = (
            "failed"
            if normalized_load >= active_policy["max_load_per_logical_core"]
            else "qualified"
        )
        load_observation: Dict[str, Any] = {
            "status": load_status,
            "source": "os.getloadavg",
            "raw": _raw_json_evidence(
                {
                    "load_average": load_values,
                    "logical_cores": logical_cores,
                }
            ),
            "parsed": {
                "load_average": load_values,
                "logical_cores": logical_cores,
                "one_minute_per_logical_core": normalized_load,
                "failure_threshold": active_policy["max_load_per_logical_core"],
            },
            "reason": "load is below threshold" if load_status == "qualified" else "load exceeds threshold",
        }
    except (AttributeError, OSError):
        load_observation = {
            "status": "failed",
            "source": "os.getloadavg",
            "reason": "load average is unavailable on this host",
            "raw": _raw_json_evidence(None),
            "parsed": None,
        }
    if sys.platform == "darwin":
        power_capture = _dynamic_command([DARWIN_PMSET, "-g", "batt"])
        power_text = power_capture.get("raw", {}).get("text", "")
        ac_power = "Now drawing from 'AC Power'" in power_text
        power = _parsed_observation(
            power_capture,
            {"ac_power": ac_power},
            power_capture.get("status") == "captured"
            and (
                ac_power
                if active_policy["require_ac_power"]
                else ("Now drawing from" in power_text)
            ),
            "AC power confirmed" if ac_power else "AC power was not confirmed",
        )
        low_capture = _dynamic_command([DARWIN_PMSET, "-g"])
        low_text = low_capture.get("raw", {}).get("text", "")
        low_match = re.search(r"^\s*lowpowermode\s+(\d+)\s*$", low_text, re.MULTILINE)
        low_enabled = None if low_match is None else int(low_match.group(1)) != 0
        low_power = _parsed_observation(
            low_capture,
            {"enabled": low_enabled},
            low_capture.get("status") == "captured"
            and low_enabled is not None
            and (active_policy["allow_low_power_mode"] or not low_enabled),
            "low-power mode parsed and allowed"
            if low_enabled is not None
            else "low-power mode could not be parsed",
        )
        frequency_capture = _dynamic_command([DARWIN_PMSET, "-g", "therm"])
        frequency_text = frequency_capture.get("raw", {}).get("text", "")
        frequency_parsed = parse_darwin_frequency_observation(
            frequency_text
        )
        frequency = _parsed_observation(
            frequency_capture,
            frequency_parsed,
            frequency_capture.get("status") == "captured"
            and dynamic_observation_is_qualified(
                "frequency_governor",
                {"parsed": frequency_parsed},
                active_policy,
                "Darwin",
                str(host.get("architecture")),
            ),
            (
                "frequency/governor control is not applicable on Apple Silicon; "
                "pmset thermal output is retained only as auxiliary raw evidence"
            ),
        )
        thermal_capture = _dynamic_command(
            [
                DARWIN_OSASCRIPT,
                "-l",
                "JavaScript",
                "-e",
                DARWIN_THERMAL_STATE_SCRIPT,
            ]
        )
        thermal_text = thermal_capture.get("raw", {}).get("text", "")
        thermal_parsed = parse_darwin_process_thermal_state(thermal_text)
        thermal_normal = thermal_parsed["normal"]
        thermal = _parsed_observation(
            thermal_capture,
            thermal_parsed,
            thermal_capture.get("status") == "captured" and thermal_normal,
            (
                "Foundation ProcessInfo thermal state is nominal"
                if thermal_normal
                else "Foundation ProcessInfo thermal state is non-nominal or unparseable"
            ),
        )
        swap_capture = _dynamic_command(
            [DARWIN_SYSCTL, "-n", "vm.swapusage"]
        )
        swap_text = swap_capture.get("raw", {}).get("text", "")
        swap_match = re.search(r"used\s*=\s*([0-9.]+)([KMGT])", swap_text)
        swap_used = (
            _parse_byte_quantity(swap_match.group(1), swap_match.group(2))
            if swap_match
            else None
        )
        swap = _parsed_observation(
            swap_capture,
            {"used_bytes": swap_used},
            swap_capture.get("status") == "captured" and swap_used is not None,
            "swap usage parsed" if swap_used is not None else "swap usage could not be parsed",
        )
        affinity: Dict[str, Any] = {
            "status": "qualified",
            "source": "system-api",
            "raw": _raw_json_evidence({"supported": False}),
            "parsed": {"supported": False, "enforced": False},
            "reason": "macOS affinity limitation was explicitly observed",
        }
    elif os.name == "nt":
        power_capture = _dynamic_command(
            [str(windows_system_directory() / "powercfg.exe"), "/getactivescheme"]
        )
        power = _parsed_observation(
            power_capture,
            None,
            False,
            "Windows AC state is not yet parsed by this authority",
        )
        low_power = _parsed_observation(
            power_capture,
            None,
            False,
            "Windows low-power state is not yet parsed by this authority",
        )
        frequency = {
            "status": "failed",
            "source": "system-api",
            "reason": "Windows frequency state is not exposed portably",
            "raw": _raw_json_evidence(None),
            "parsed": None,
        }
        thermal = {
            "status": "failed",
            "source": "system-api",
            "reason": "Windows thermal state is not exposed portably",
            "raw": _raw_json_evidence(None),
            "parsed": None,
        }
        swap = {
            "status": "failed",
            "source": "GlobalMemoryStatusEx",
            "raw": _raw_json_evidence(memory_bytes()),
            "parsed": None,
            "reason": "memory status captured but swap usage is unavailable",
        }
        affinity = {
            "status": "failed",
            "source": "system-api",
            "reason": "affinity mask capture is unavailable in this collector version",
            "raw": _raw_json_evidence(None),
            "parsed": None,
        }
    else:
        governor_paths = sorted(
            Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor")
        )
        governor_values = [
            path.read_text(encoding="utf-8", errors="replace").strip()
            for path in governor_paths
        ]
        governors_qualified = bool(governor_values) and all(
            value in active_policy["allowed_linux_governors"]
            for value in governor_values
        )
        frequency = {
            "status": "qualified" if governors_qualified else "failed",
            "source": "sysfs",
            "raw": _raw_json_evidence(governor_values),
            "parsed": {"governors": governor_values},
            "reason": "governors are allowed" if governors_qualified else "governors are missing or not allowed",
        }
        power_paths = sorted(Path("/sys/class/power_supply").glob("*/online"))
        power_values = {
            str(path): path.read_text(encoding="utf-8", errors="replace").strip()
            for path in power_paths
        }
        ac_power = bool(power_values) and any(
            value == "1" for value in power_values.values()
        )
        power = {
            "status": (
                "qualified"
                if power_values
                and (ac_power or not active_policy["require_ac_power"])
                else "failed"
            ),
            "source": "sysfs",
            "reason": (
                "AC power confirmed"
                if ac_power
                else "AC power source was missing, offline, or unparseable"
            ),
            "raw": _raw_json_evidence(power_values),
            "parsed": {"ac_power": ac_power if power_values else None},
        }
        low_power_enabled = (
            None
            if not governor_values
            else any(value != "performance" for value in governor_values)
        )
        low_power_qualified = low_power_enabled is not None and (
            active_policy["allow_low_power_mode"] or not low_power_enabled
        )
        low_power = {
            **frequency,
            "status": "qualified" if low_power_qualified else "failed",
            "parsed": {"enabled": low_power_enabled},
            "reason": (
                "performance governors indicate low-power mode is disabled"
                if low_power_qualified
                else "low-power state is enabled or unparseable"
            ),
        }
        thermal_paths = sorted(Path("/sys/class/thermal").glob("thermal_zone*/temp"))
        thermal_values = [float(path.read_text().strip()) / 1000.0 for path in thermal_paths]
        thermal_qualified = bool(thermal_values) and max(thermal_values) <= active_policy["max_thermal_celsius"]
        thermal = {
            "status": "qualified" if thermal_qualified else "failed",
            "source": "sysfs",
            "raw": _raw_json_evidence(thermal_values),
            "parsed": {"temperatures_celsius": thermal_values, "maximum_celsius": max(thermal_values) if thermal_values else None},
            "reason": "thermal values are below threshold" if thermal_qualified else "thermal values missing or above threshold",
        }
        try:
            swap_text = Path("/proc/meminfo").read_text(
                encoding="utf-8", errors="replace"
            )
            swap_capture = {
                "status": "captured",
                "source": "procfs",
                "raw": _raw_text_evidence(swap_text),
            }
        except OSError as error:
            swap_text = ""
            swap_capture = {
                "status": "unavailable",
                "source": "procfs",
                "raw": _raw_text_evidence(""),
                "reason": str(error),
            }
        swap_match = re.search(r"SwapTotal:\s*(\d+)\s*kB.*SwapFree:\s*(\d+)\s*kB", swap_text, re.DOTALL)
        swap_used = (
            (int(swap_match.group(1)) - int(swap_match.group(2))) * 1024
            if swap_match
            else None
        )
        swap = _parsed_observation(
            swap_capture,
            {"used_bytes": swap_used},
            swap_capture.get("status") == "captured" and swap_used is not None,
            "swap usage parsed" if swap_used is not None else "swap usage could not be parsed",
        )
        affinity = (
            {
                "status": "qualified",
                "source": "os.sched_getaffinity",
                "raw": _raw_json_evidence(sorted(os.sched_getaffinity(0))),
                "parsed": {"cpus": sorted(os.sched_getaffinity(0))},
                "reason": "affinity mask captured",
            }
            if hasattr(os, "sched_getaffinity")
            else {
                "status": "failed",
                "source": "system-api",
                "reason": "process affinity API unavailable",
                "raw": _raw_json_evidence(None),
                "parsed": None,
            }
        )
    observations = {
        "power_mode": power,
        "low_power_mode": low_power,
        "frequency_governor": frequency,
        "thermal_state": thermal,
        "concurrent_load": load_observation,
        "swap": swap,
        "affinity": affinity,
    }
    host_sha = canonical_json_sha256(host)
    eligible = host_sha == authority_host_sha256 and all(
        observation["status"] == "qualified" for observation in observations.values()
    )
    body = {
        "schema": 1,
        "captured_at_utc": captured_at,
        "monotonic_ns": monotonic_ns,
        "authority_host_sha256": authority_host_sha256,
        "observed_host_sha256": host_sha,
        "observations": observations,
        "policy": active_policy,
        "eligible": eligible,
        "reason": (
            "dynamic host state was captured without a disqualifying anomaly"
            if eligible
            else "dynamic host state or canonical-host binding failed"
        ),
    }
    return {
        **body,
        "snapshot_sha256": canonical_json_sha256(body),
    }


def canonical_json_sha256(value: Any) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("utf-8")
    return v1.sha256_bytes(encoded)


def frozen_source_lock(manifest: Dict[str, Any]) -> list[Dict[str, Any]]:
    return [
        {
            "id": workload["id"],
            "performance_input": workload["performance_input"],
            "fixture_sha256": workload["fixtures"]["performance"]["sha256"],
            "source_sha256": {
                name: source["sha256"]
                for name, source in workload["sources"].items()
            },
            "cpp_derived_from": workload["sources"]["cpp"]["derived_from"],
            "cpp_derivation": workload["sources"]["cpp"]["derivation"],
            "cpp_allocation_mappings": workload["sources"]["cpp"][
                "allocation_mappings"
            ],
            "allocation_contract_rfc_commit": manifest["rfc"][
                "allocation_clarification_merge_commit"
            ],
        }
        for workload in manifest["workloads"]
    ]


def stable_toolchain_identity(toolchains: Dict[str, Any]) -> Dict[str, Any]:
    identity = {}
    fields_by_tool = {
        "nomo": ("path", "realpath", "sha256", "version", "version_output"),
        "clang": (
            "path",
            "invocation_realpath",
            "invocation_sha256",
            "selected_path",
            "realpath",
            "sha256",
            "version",
            "version_output",
            "installation",
            "target_triple",
            "driver_config_flags",
        ),
        "clangxx": (
            "path",
            "invocation_realpath",
            "invocation_sha256",
            "selected_path",
            "realpath",
            "sha256",
            "version",
            "version_output",
            "installation",
            "target_triple",
            "driver_config_flags",
        ),
        "go": (
            "path",
            "realpath",
            "sha256",
            "version",
            "version_output",
            "goroot",
        ),
    }
    for tool, fields in fields_by_tool.items():
        record = toolchains.get(tool, {})
        identity[tool] = {field: record.get(field) for field in fields}
    identity["compiler_contract"] = {
        "driver_config_flags": list(CLANG_DRIVER_CONFIG_FLAGS),
        "c_flags": list(BASE_C_FLAGS),
        "cpp_flags": list(BASE_CPP_FLAGS),
        "forbidden_flags": list(FORBIDDEN_COMPILER_FLAG_PREFIXES),
        "environment": toolchains.get("build_environment"),
        "runtime_environments": toolchains.get("runtime_environments"),
    }
    return identity


def qualification_bindings(
    host: Dict[str, Any],
    toolchains: Dict[str, Any],
    source_lock: Sequence[Dict[str, Any]],
    release_lanes: Dict[str, Any],
    prepared_bundle_sha256: Optional[str] = None,
) -> Dict[str, Any]:
    bindings = {
        "authority_host_sha256": canonical_json_sha256(host),
        "reference_toolchains_sha256": canonical_json_sha256(
            stable_toolchain_identity(toolchains)
        ),
        "frozen_source_lock_sha256": canonical_json_sha256(source_lock),
        "candidate_commit": release_lanes.get("candidate", {}).get(
            "expected_commit"
        ),
        "candidate_nomo_sha256": release_lanes.get("candidate", {}).get(
            "nomo_sha256"
        ),
        "main_commit": release_lanes.get("main", {}).get("expected_commit"),
        "main_nomo_sha256": release_lanes.get("main", {}).get("nomo_sha256"),
    }
    if prepared_bundle_sha256 is not None:
        bindings["prepared_bundle_sha256"] = prepared_bundle_sha256
    return bindings


def environment_qualification(
    manifest: Dict[str, Any],
    qualification_path: Optional[str],
    expected_bindings: Dict[str, Any],
) -> Dict[str, Any]:
    required = manifest["environment_qualification"]["required_checks"]
    if qualification_path is None:
        return {
            "kind": "canonical-host-static-authorization-v1",
            "status": "ineligible",
            "eligible": False,
            "policy": "fail-closed",
            "qualification_path": None,
            "qualification_sha256": None,
            "checks": {},
            "missing_or_unqualified": list(required),
            "provided_bindings": None,
            "expected_bindings": expected_bindings,
            "binding_mismatches": sorted(expected_bindings),
            "dynamic_policy": DYNAMIC_ENVIRONMENT_POLICY,
            "reason": "no canonical-host qualification record was supplied",
        }
    path = Path(qualification_path).resolve()
    record = read_json_strict(path)
    return derive_environment_qualification(
        manifest,
        record,
        str(path),
        v1.sha256_file(path),
        expected_bindings,
    )


def derive_environment_qualification(
    manifest: Dict[str, Any],
    record: Dict[str, Any],
    qualification_path: str,
    qualification_sha256: str,
    expected_bindings: Dict[str, Any],
) -> Dict[str, Any]:
    required = manifest["environment_qualification"]["required_checks"]
    validate_json_schema(
        record,
        DEFAULT_MANIFEST.parent / "schema" / "environment-v2.schema.json",
        "environment qualification",
    )
    if record.get("schema") != 1:
        raise HarnessError("environment qualification schema must be 1")
    if record.get("dynamic_policy") != DYNAMIC_ENVIRONMENT_POLICY:
        raise HarnessError("environment qualification dynamic policy changed")
    canonical_host_id = record.get("canonical_host_id")
    captured_at_utc = record.get("captured_at_utc")
    if not isinstance(canonical_host_id, str) or not canonical_host_id:
        raise HarnessError("environment qualification needs canonical_host_id")
    if not isinstance(captured_at_utc, str) or not captured_at_utc:
        raise HarnessError("environment qualification needs captured_at_utc")
    checks = record.get("checks")
    if not isinstance(checks, dict):
        raise HarnessError("environment qualification checks must be an object")
    missing_or_unqualified = []
    for check_id in required:
        check = checks.get(check_id)
        if (
            not isinstance(check, dict)
            or check.get("status") != "qualified"
            or check.get("value") in (None, "")
            or not isinstance(check.get("source"), str)
            or not check.get("source")
            or not isinstance(check.get("evidence"), dict)
            or check["evidence"].get("value_sha256")
            != canonical_json_sha256(check.get("value"))
        ):
            missing_or_unqualified.append(check_id)
    provided_bindings = record.get("bindings")
    if not isinstance(provided_bindings, dict):
        provided_bindings = {}
    binding_mismatches = [
        key
        for key, expected in expected_bindings.items()
        if provided_bindings.get(key) != expected
    ]
    expected_check_values = {
        "canonical_host_identity": expected_bindings["authority_host_sha256"],
        "toolchain_identity": expected_bindings["reference_toolchains_sha256"],
        "frozen_source_lock": expected_bindings["frozen_source_lock_sha256"],
    }
    for check_id, expected_value in expected_check_values.items():
        if checks.get(check_id, {}).get("value") != expected_value:
            binding_mismatches.append(f"checks.{check_id}")
    eligible = not missing_or_unqualified and not binding_mismatches
    return {
        "kind": "canonical-host-static-authorization-v1",
        "status": "eligible" if eligible else "ineligible",
        "eligible": eligible,
        "policy": "fail-closed",
        "qualification_path": qualification_path,
        "qualification_sha256": qualification_sha256,
        "canonical_host_id": canonical_host_id,
        "captured_at_utc": captured_at_utc,
        "checks": checks,
        "missing_or_unqualified": missing_or_unqualified,
        "provided_bindings": provided_bindings,
        "expected_bindings": expected_bindings,
        "binding_mismatches": binding_mismatches,
        "dynamic_policy": record["dynamic_policy"],
        "reason": (
            "all RFC 0043 checks are qualified and bound to actual host, "
            "toolchains, commits, binaries, and source lock"
            if eligible
            else "required checks are missing, unqualified, or bound to different evidence"
        ),
    }


def binary_path(root: Path, name: str) -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    return root / f"{name}{suffix}"


def artifact_binary_path(
    root: PurePath, name: str, host_os: str
) -> PurePath:
    suffix = ".exe" if host_os == "Windows" else ""
    return root / f"{name}{suffix}"


def path_lexically_exists(path: Path) -> bool:
    return path.exists() or path.is_symlink()


def lexical_absolute_path(path: Path) -> Path:
    """Return an absolute path without following its final component or parents."""
    return Path(os.path.abspath(os.fspath(path.expanduser())))


def lexical_path_lstat(path: Path) -> Optional[os.stat_result]:
    try:
        return path.lstat()
    except FileNotFoundError:
        return None
    except OSError as error:
        raise HarnessError(f"cannot inspect lexical path {path}: {error}") from error


def lexical_path_is_link(path: Path, metadata: os.stat_result) -> bool:
    if stat.S_ISLNK(metadata.st_mode):
        return True
    is_junction = getattr(path, "is_junction", None)
    return bool(is_junction is not None and is_junction())


def validate_lexical_path_chain(
    path: Path,
    label: str,
    *,
    final_kind: str,
) -> Path:
    """Validate a target without resolving a dangling link or linked parent."""
    target = lexical_absolute_path(path)
    metadata = lexical_path_lstat(target)
    if metadata is not None:
        if lexical_path_is_link(target, metadata):
            raise HarnessError(f"{label} is a symlink or junction: {target}")
        if final_kind == "absent":
            raise HarnessError(
                f"{label} already exists; choose a fresh evidence path: {target}"
            )
        if final_kind == "directory" and not stat.S_ISDIR(metadata.st_mode):
            raise HarnessError(f"{label} is not a directory: {target}")
        if final_kind == "regular" and not stat.S_ISREG(metadata.st_mode):
            raise HarnessError(f"{label} is not a regular file: {target}")
    elif final_kind != "absent":
        raise HarnessError(f"{label} does not exist: {target}")

    for parent in target.parents:
        parent_metadata = lexical_path_lstat(parent)
        if parent_metadata is None:
            continue
        if lexical_path_is_link(parent, parent_metadata):
            raise HarnessError(
                f"{label} has a symlink or junction parent: {parent}"
            )
        if not stat.S_ISDIR(parent_metadata.st_mode):
            raise HarnessError(
                f"{label} has a non-directory parent: {parent}"
            )
    return target


_FILESYSTEM_CASE_SENSITIVITY: Dict[Tuple[int, int, str], bool] = {}


def nearest_existing_lexical_directory(path: Path) -> Path:
    target = lexical_absolute_path(path)
    for parent in target.parents:
        metadata = lexical_path_lstat(parent)
        if metadata is None:
            continue
        if lexical_path_is_link(parent, metadata):
            raise HarnessError(
                f"cannot determine path identity through symlink or junction: {parent}"
            )
        if not stat.S_ISDIR(metadata.st_mode):
            raise HarnessError(
                f"cannot determine path identity through non-directory: {parent}"
            )
        return parent
    raise HarnessError(f"cannot find an existing filesystem for path: {target}")


def nearest_existing_lexical_path(path: Path) -> Path:
    target = lexical_absolute_path(path)
    for candidate in (target, *target.parents):
        metadata = lexical_path_lstat(candidate)
        if metadata is None:
            continue
        if lexical_path_is_link(candidate, metadata):
            raise HarnessError(
                "cannot determine path identity through symlink or junction: "
                f"{candidate}"
            )
        return candidate
    raise HarnessError(f"cannot find an existing filesystem for path: {target}")


def filesystem_is_case_sensitive(path: Path) -> bool:
    """Probe the target filesystem without retaining a file or directory."""
    directory = nearest_existing_lexical_directory(path)
    metadata = directory.lstat()
    cache_key = (
        int(metadata.st_dev),
        int(metadata.st_ino),
        str(directory),
    )
    cached = _FILESYSTEM_CASE_SENSITIVITY.get(cache_key)
    if cached is not None:
        return cached

    probe_name = f".nomo-benchmarksgame-v2-case-{uuid.uuid4().hex}a"
    probe = directory / probe_name
    alias = directory / f"{probe_name[:-1]}A"
    descriptor: Optional[int] = None
    try:
        descriptor = os.open(
            probe,
            os.O_CREAT | os.O_EXCL | os.O_WRONLY,
            0o600,
        )
        alias_metadata = lexical_path_lstat(alias)
        case_sensitive = alias_metadata is None
    except OSError as error:
        raise HarnessError(
            f"cannot safely determine filesystem case sensitivity in {directory}: "
            f"{error}"
        ) from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
        try:
            probe.unlink()
        except FileNotFoundError:
            pass
        except OSError as error:
            raise HarnessError(
                f"cannot remove filesystem case-sensitivity probe {probe}: {error}"
            ) from error
    _FILESYSTEM_CASE_SENSITIVITY[cache_key] = case_sensitive
    return case_sensitive


def windows_long_path_name(
    path: Path,
    kernel32: Optional[Any] = None,
) -> Path:
    """Expand an existing Windows path through the authoritative Win32 API."""
    if kernel32 is None:
        if os.name != "nt":
            raise HarnessError("GetLongPathNameW is available only on Windows")
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetLongPathNameW.argtypes = [
        wintypes.LPCWSTR,
        wintypes.LPWSTR,
        wintypes.DWORD,
    ]
    kernel32.GetLongPathNameW.restype = wintypes.DWORD
    source = lexical_absolute_path(path)
    buffer = ctypes.create_unicode_buffer(32768)
    length = int(
        kernel32.GetLongPathNameW(
            str(source),
            buffer,
            len(buffer),
        )
    )
    if length == 0 or length >= len(buffer):
        raise HarnessError(f"GetLongPathNameW failed for path identity: {source}")
    expanded = lexical_absolute_path(Path(buffer.value))
    if not expanded.is_absolute():
        raise HarnessError(
            f"GetLongPathNameW returned a non-absolute path: {expanded}"
        )
    return expanded


def filesystem_identity_path(path: Path, existing_prefix: Path) -> Path:
    target = lexical_absolute_path(path)
    if os.name != "nt":
        return target
    expanded_prefix = windows_long_path_name(existing_prefix)
    try:
        suffix = target.relative_to(existing_prefix)
    except ValueError as error:
        raise HarnessError(
            f"Windows path identity prefix changed: {target} vs {existing_prefix}"
        ) from error
    return expanded_prefix.joinpath(*suffix.parts)


def filesystem_path_identity(path: Path) -> Tuple[int, Tuple[str, ...]]:
    target = lexical_absolute_path(path)
    existing_prefix = nearest_existing_lexical_path(target)
    metadata = existing_prefix.lstat()
    case_sensitive = filesystem_is_case_sensitive(target)
    identity_path = filesystem_identity_path(target, existing_prefix)
    parts = tuple(
        normalized
        if case_sensitive
        else normalized.casefold()
        for part in identity_path.parts
        for normalized in (unicodedata.normalize("NFC", part),)
    )
    return int(metadata.st_dev), parts


def filesystem_paths_overlap(left: Path, right: Path) -> bool:
    left_device, left_parts = filesystem_path_identity(left)
    right_device, right_parts = filesystem_path_identity(right)
    if left_device != right_device:
        return False
    shortest = min(len(left_parts), len(right_parts))
    return left_parts[:shortest] == right_parts[:shortest]


def require_absent_build_path(path: Path, label: str) -> None:
    if path_lexically_exists(path):
        raise HarnessError(
            f"{label} already exists; refusing to reuse stale build evidence: "
            f"{path.resolve()}"
        )


def require_new_build_file(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise HarnessError(
            f"{label} did not create a new regular file: {path.resolve()}"
        )


def math_flags_for_host(
    workload: Dict[str, Any],
    host_os: str,
) -> list[str]:
    if host_os not in {"Darwin", "Linux", "Windows"}:
        raise HarnessError(f"unsupported build host OS: {host_os}")
    if workload.get("link_math") and host_os != "Windows":
        return ["-lm"]
    return []


def math_flags(workload: Dict[str, Any]) -> list[str]:
    return math_flags_for_host(workload, platform.system())


def parse_project_name(manifest_path: Path) -> str:
    match = re.search(
        r'^name\s*=\s*"([^"]+)"',
        manifest_path.read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    if match is None:
        raise HarnessError(f"cannot find package name in {manifest_path}")
    return match.group(1)


def build_reference_workload(
    workload: Dict[str, Any],
    suite_root: Path,
    bundle_root: Path,
    toolchains: Dict[str, Any],
    build_timeout_seconds: float,
    include_nomo_baseline: bool,
) -> Tuple[Dict[str, Any], Dict[str, Path]]:
    workload_id = workload["id"]
    build_root = bundle_root / "build" / workload_id / "references"
    binary_root = bundle_root / "bin"
    require_absent_build_path(build_root, f"{workload_id} reference build root")
    build_root.mkdir(parents=True, exist_ok=False)
    binary_root.mkdir(parents=True, exist_ok=True)
    sources = workload["sources"]
    binaries: Dict[str, Path] = {}
    commands: Dict[str, Dict[str, Any]] = {}
    source_files = {}
    compiler_output = {}

    source_paths = {
        lane: resolve_suite_path(
            suite_root, sources[lane]["path"], f"{workload_id} {lane} source"
        )
        for lane in REFERENCE_LANES
    }
    copied_sources = {}
    extensions = {"c": ".c", "cpp": ".cpp", "semantic-c": ".c", "go": ".go"}
    for lane, source in source_paths.items():
        copied = build_root / f"{lane}{extensions[lane]}"
        shutil.copy2(source, copied)
        copied_sources[lane] = copied
        source_files[lane] = {"path": str(source), "sha256": v1.sha256_file(source)}

    build_specs = {
        "c": [
            toolchains["clang"]["path"],
            *CLANG_DRIVER_CONFIG_FLAGS,
            *BASE_C_FLAGS,
            str(copied_sources["c"]),
            "-o",
            str(binary_path(binary_root, f"{workload_id}-c")),
            *math_flags(workload),
        ],
        "cpp": [
            toolchains["clangxx"]["path"],
            *CLANG_DRIVER_CONFIG_FLAGS,
            *BASE_CPP_FLAGS,
            str(copied_sources["cpp"]),
            "-o",
            str(binary_path(binary_root, f"{workload_id}-cpp")),
            *math_flags(workload),
        ],
        "semantic-c": [
            toolchains["clang"]["path"],
            *CLANG_DRIVER_CONFIG_FLAGS,
            *BASE_C_FLAGS,
            str(copied_sources["semantic-c"]),
            "-o",
            str(binary_path(binary_root, f"{workload_id}-semantic-c")),
            *math_flags(workload),
        ],
        "go": [
            toolchains["go"]["path"],
            "build",
            "-o",
            str(binary_path(binary_root, f"{workload_id}-go")),
            str(copied_sources["go"]),
        ],
    }
    for lane, command in build_specs.items():
        output = Path(command[command.index("-o") + 1]).resolve()
        require_absent_build_path(output, f"{workload_id} {lane} output")
        try:
            if lane == "go":
                with isolated_go_build_cache(build_root) as go_cache_environment:
                    record, stdout, stderr = run_build_capture(
                        command,
                        timeout_seconds=build_timeout_seconds,
                        cwd=REPOSITORY_ROOT,
                        approved_environment_overrides=go_cache_environment,
                    )
            else:
                record, stdout, stderr = run_build_capture(
                    command,
                    timeout_seconds=build_timeout_seconds,
                    cwd=REPOSITORY_ROOT,
                )
        except BuildCollectionError as error:
            raise WorkloadBuildError(
                str(error),
                workload_build_failure_record(
                    workload_id,
                    lane,
                    "reference-build",
                    copied_sources[lane],
                    output,
                    error.record,
                ),
            ) from error
        commands[f"{lane}_build"] = record
        compiler_output[lane] = {
            "stdout": stdout.decode("utf-8", errors="replace"),
            "stderr": stderr.decode("utf-8", errors="replace"),
        }
        require_regular_workload_build_output(
            workload_id=workload_id,
            lane=lane,
            phase="reference-build",
            source=copied_sources[lane],
            output=output,
            command=record,
            stdout=stdout,
            stderr=stderr,
        )
        require_new_build_file(output, f"{workload_id} {lane} output")
        binaries[lane] = output

    generated_c = None
    if include_nomo_baseline:
        nomo_source = resolve_suite_path(
            suite_root, sources["nomo"]["path"], f"{workload_id} Nomo source"
        )
        nomo_manifest = resolve_suite_path(
            suite_root,
            sources["nomo"]["project_manifest"],
            f"{workload_id} Nomo manifest",
        )
        nomo_project = build_root / "nomo-baseline-project"
        require_absent_build_path(
            nomo_project, f"{workload_id} Nomo baseline project"
        )
        v1.copy_nomo_project(nomo_source, nomo_manifest, nomo_project)
        emit_command = [
            toolchains["nomo"]["path"],
            "build",
            str(nomo_project),
            "--emit-c",
        ]
        generated_c = nomo_project / "build" / "c" / "main.c"
        require_absent_build_path(
            generated_c, f"{workload_id} Nomo baseline generated C"
        )
        try:
            emit_record, emit_stdout, emit_stderr = run_build_capture(
                emit_command,
                timeout_seconds=build_timeout_seconds,
                cwd=REPOSITORY_ROOT,
            )
        except BuildCollectionError as error:
            raise WorkloadBuildError(
                str(error),
                workload_build_failure_record(
                    workload_id,
                    "nomo-baseline",
                    "nomo-emit-c",
                    nomo_source,
                    generated_c,
                    error.record,
                ),
            ) from error
        require_regular_workload_build_output(
            workload_id=workload_id,
            lane="nomo-baseline",
            phase="nomo-emit-c",
            source=nomo_source,
            output=generated_c,
            command=emit_record,
            stdout=emit_stdout,
            stderr=emit_stderr,
        )
        require_new_build_file(
            generated_c, f"{workload_id} Nomo baseline generated C"
        )
        nomo_binary = binary_path(binary_root, f"{workload_id}-nomo-baseline")
        require_absent_build_path(
            nomo_binary, f"{workload_id} Nomo baseline binary"
        )
        clang_command = [
            toolchains["clang"]["path"],
            *CLANG_DRIVER_CONFIG_FLAGS,
            *BASE_C_FLAGS,
            str(generated_c),
            "-o",
            str(nomo_binary),
            *math_flags(workload),
        ]
        try:
            clang_record, clang_stdout, clang_stderr = run_build_capture(
                clang_command,
                timeout_seconds=build_timeout_seconds,
                cwd=REPOSITORY_ROOT,
            )
        except BuildCollectionError as error:
            raise WorkloadBuildError(
                str(error),
                workload_build_failure_record(
                    workload_id,
                    "nomo-baseline",
                    "nomo-generated-c-clang",
                    generated_c,
                    nomo_binary,
                    error.record,
                ),
            ) from error
        require_regular_workload_build_output(
            workload_id=workload_id,
            lane="nomo-baseline",
            phase="nomo-generated-c-clang",
            source=generated_c,
            output=nomo_binary,
            command=clang_record,
            stdout=clang_stdout,
            stderr=clang_stderr,
        )
        require_new_build_file(
            nomo_binary, f"{workload_id} Nomo baseline binary"
        )
        binaries["nomo-baseline"] = nomo_binary.resolve()
        commands["nomo_baseline_emit_c"] = emit_record
        commands["nomo_baseline_clang"] = clang_record
        compiler_output["nomo-baseline"] = {
            "emit_stdout": emit_stdout.decode("utf-8", errors="replace"),
            "emit_stderr": emit_stderr.decode("utf-8", errors="replace"),
            "clang_stdout": clang_stdout.decode("utf-8", errors="replace"),
            "clang_stderr": clang_stderr.decode("utf-8", errors="replace"),
        }
        source_files["nomo"] = [
            {"path": str(nomo_source), "sha256": v1.sha256_file(nomo_source)},
            {"path": str(nomo_manifest), "sha256": v1.sha256_file(nomo_manifest)},
        ]

    for lane, path in binaries.items():
        binaries[lane] = path.resolve()
    return {
        "kind": "reference-and-correctness-baseline",
        "source_files": source_files,
        "compiled_sources": {
            lane: {"path": str(path.resolve()), "sha256": v1.sha256_file(path)}
            for lane, path in copied_sources.items()
        },
        "commands": commands,
        "compiler_output": compiler_output,
        "generated_c": (
            {
                "path": str(generated_c.resolve()),
                "sha256": v1.sha256_file(generated_c),
                "unmodified_after_emit": True,
                "decisional_release_lane": False,
            }
            if generated_c is not None
            else None
        ),
        "binaries": {
            lane: {"path": str(path), "sha256": v1.sha256_file(path)}
            for lane, path in binaries.items()
        },
        "compile_time_excluded_from_run_time": True,
    }, binaries


def release_lane_state(
    checkout_argument: Optional[str],
    commit_argument: Optional[str],
    label: str,
    bundle_root: Path,
    build_timeout_seconds: float,
    cargo_argument: str,
    require_origin_main: bool,
) -> Dict[str, Any]:
    if checkout_argument is None or commit_argument is None:
        return {
            "label": label,
            "status": "unavailable",
            "reason": "detached checkout and exact commit must both be supplied",
            "emit_c_fallback_used": False,
        }
    if re.fullmatch(r"[0-9a-f]{40}", commit_argument) is None:
        return {
            "label": label,
            "status": "unavailable",
            "reason": "expected commit must be a full lowercase 40-hex Git commit",
            "emit_c_fallback_used": False,
        }
    checkout = Path(checkout_argument).resolve()
    if not (checkout / ".git").exists():
        return {
            "label": label,
            "status": "unavailable",
            "reason": f"not a Git checkout: {checkout}",
            "emit_c_fallback_used": False,
        }
    try:
        repository_before = v1.repository_state(checkout, require_clean=True)
        origin_url = v1.git_capture(checkout, ["remote", "get-url", "origin"])
        normalized_origin = normalize_nomo_origin(origin_url)
        if normalized_origin != "github.com/nomo-lang/nomo":
            raise HarnessError(
                f"{label} origin is not the official nomo-lang/nomo repository"
            )
        v1.git_capture(checkout, ["cat-file", "-e", f"{commit_argument}^{{commit}}"])
        branch = v1.git_capture(checkout, ["rev-parse", "--abbrev-ref", "HEAD"])
        if branch != "HEAD":
            raise HarnessError(
                f"{label} checkout must be detached, found branch {branch!r}"
            )
        if repository_before["commit"] != commit_argument:
            raise HarnessError(
                f"{label} checkout commit mismatch: expected {commit_argument}, "
                f"found {repository_before['commit']}"
            )
        origin_main = None
        remote_main = None
        remote_refs = v1.git_capture(checkout, ["ls-remote", "origin"])
        if not any(
            line.split(maxsplit=1)[0] == commit_argument
            for line in remote_refs.splitlines()
            if line.strip()
        ):
            raise HarnessError(
                f"{label} commit is not advertised by the official origin"
            )
        if require_origin_main:
            origin_main = v1.git_capture(checkout, ["rev-parse", "origin/main"])
            remote_line = v1.git_capture(
                checkout, ["ls-remote", "origin", "refs/heads/main"]
            )
            fields = remote_line.split()
            remote_main = fields[0] if len(fields) == 2 else None
            if origin_main != commit_argument or remote_main != commit_argument:
                raise HarnessError(
                    f"{label} must bind current origin/main: expected {commit_argument}, "
                    f"local origin/main {origin_main}, remote main {remote_main}"
                )
        cargo_invocation = resolve_executable(cargo_argument, "Cargo")
        cargo_configs = cargo_config_provenance(checkout)
        target_dir = bundle_root / "compiler-build" / label
        cargo_home = (
            bundle_root / "compiler-build" / f"{label}-cargo-home"
        )
        if path_lexically_exists(target_dir):
            raise HarnessError(
                f"{label} isolated compiler target already exists; use a fresh output path"
            )
        if path_lexically_exists(cargo_home):
            raise HarnessError(
                f"{label} isolated CARGO_HOME already exists; use a fresh output path"
            )
        cargo_home.mkdir(parents=True)
        resolution_environment = {
            "CARGO_TARGET_DIR": str(target_dir.resolve()),
            "CARGO_HOME": str(cargo_home.resolve()),
        }
        cargo, rustc, rustup_resolution = resolve_rustup_toolchain(
            cargo_invocation,
            checkout,
            resolution_environment,
        )
        cargo_environment = {
            **resolution_environment,
            "RUSTC": str(rustc),
        }
        command = [str(cargo), "build", "--locked", "--release", "--bin", "nomo"]
        nomo = binary_path(target_dir / "release", "nomo")
        require_absent_build_path(nomo, f"{label} self-built Nomo compiler")
        try:
            cargo_version_command, cargo_version_stdout, _ = run_build_capture(
                [str(cargo), "--version"],
                timeout_seconds=30.0,
                cwd=checkout,
                approved_environment_overrides=cargo_environment,
            )
            cargo_version = cargo_version_stdout.decode(
                "utf-8", errors="strict"
            ).strip()
            if not cargo_version.startswith("cargo "):
                raise HarnessError("Cargo version probe did not report Cargo")
            rustc_record = rustc_authority(
                rustc,
                checkout,
                cargo_environment,
            )
            build_record, build_stdout, build_stderr = run_build_capture(
                command,
                timeout_seconds=build_timeout_seconds,
                cwd=checkout,
                approved_environment_overrides=cargo_environment,
            )
        finally:
            try:
                shutil.rmtree(cargo_home)
            except OSError as error:
                raise HarnessError(
                    f"{label} isolated CARGO_HOME cleanup failed: {error}"
                ) from error
        if not nomo.is_file():
            raise HarnessError(f"{label} compiler build did not produce {nomo}")
        require_new_build_file(nomo, f"{label} self-built Nomo compiler")
        repository_after = v1.repository_state(checkout, require_clean=True)
        if repository_after != repository_before:
            raise HarnessError(f"{label} checkout changed while building Nomo")
    except HarnessError as error:
        return {
            "label": label,
            "status": "unavailable",
            "reason": str(error),
            "emit_c_fallback_used": False,
        }
    release_probe = release_capability(nomo, label)
    emit_c_probe = emit_c_capability(nomo, label)
    return {
        "label": label,
        "status": "available",
        "reason": "compiler was reproducibly built from the exact clean detached checkout",
        "emit_c_fallback_used": False,
        "checkout": str(checkout),
        "expected_commit": commit_argument,
        "repository": repository_after,
        "detached_head": True,
        "origin_url": origin_url,
        "normalized_origin": normalized_origin,
        "nomo_path": str(nomo.resolve()),
        "nomo_sha256": v1.sha256_file(nomo),
        "compiler_build": {
            "repository_before": repository_before,
            "repository_after": repository_after,
            "expected_commit": commit_argument,
            "detached_head": True,
            "origin_url": origin_url,
            "normalized_origin": normalized_origin,
            "origin_main_commit": origin_main,
            "remote_main_commit": remote_main,
            "command": build_record,
            "environment": cargo_environment,
            "cargo_configs": cargo_configs,
            "rustup_resolution": rustup_resolution,
            "cargo": {
                "path": str(cargo),
                "realpath": str(cargo.resolve()),
                "version_output": cargo_version,
                "version_command": cargo_version_command,
                "sha256": v1.sha256_file(cargo.resolve()),
            },
            "rustc": rustc_record,
            "binary": {
                "path": str(nomo.resolve()),
                "sha256": v1.sha256_file(nomo),
            },
            "stdout": build_stdout.decode("utf-8", errors="replace"),
            "stderr": build_stderr.decode("utf-8", errors="replace"),
        },
        "capabilities": {
            "release": release_probe,
            "emit-c": emit_c_probe,
        },
    }


def normalize_nomo_origin(value: str) -> Optional[str]:
    normalized = value.strip().removesuffix("/").removesuffix(".git")
    accepted = (
        r"^git@github\.com:nomo-lang/nomo$",
        r"^ssh://git@github\.com/nomo-lang/nomo$",
        r"^https://github\.com/nomo-lang/nomo$",
    )
    if any(re.fullmatch(pattern, normalized, re.IGNORECASE) for pattern in accepted):
        return "github.com/nomo-lang/nomo"
    return None


def formal_project_provenance(
    project: Path,
    nomo_source: Path,
    nomo_manifest: Path,
    source_relative_path: str,
    manifest_relative_path: str,
    lane_state: Dict[str, Any],
) -> Dict[str, Any]:
    project_root = nomo_manifest.parent
    copied_source = project / nomo_source.relative_to(project_root)
    copied_manifest = project / nomo_manifest.relative_to(project_root)
    for path, label in (
        (copied_source, "copied Nomo source"),
        (copied_manifest, "copied Nomo project manifest"),
    ):
        if not path.is_file():
            raise HarnessError(f"{label} is missing from formal project: {path}")
    return {
        "path": str(project.resolve()),
        "source_relative_path": source_relative_path,
        "source": {
            "path": str(nomo_source),
            "sha256": v1.sha256_file(nomo_source),
        },
        "project_manifest_relative_path": manifest_relative_path,
        "project_manifest": {
            "path": str(nomo_manifest),
            "sha256": v1.sha256_file(nomo_manifest),
        },
        "copied_source": {
            "path": str(copied_source.resolve()),
            "sha256": v1.sha256_file(copied_source),
        },
        "copied_project_manifest": {
            "path": str(copied_manifest.resolve()),
            "sha256": v1.sha256_file(copied_manifest),
        },
        "compiler_checkout": str(Path(lane_state["checkout"]).resolve()),
        "compiler_commit": lane_state["expected_commit"],
    }


def build_release_lane(
    workload: Dict[str, Any],
    suite_root: Path,
    bundle_root: Path,
    lane: str,
    lane_state: Dict[str, Any],
    toolchains: Dict[str, Any],
    build_timeout_seconds: float,
) -> Tuple[Dict[str, Any], Path]:
    if lane_state.get("status") != "available":
        raise HarnessError(f"{lane} release lane is unavailable")
    if (
        lane_state.get("capabilities", {})
        .get("release", {})
        .get("status")
        != "available"
    ):
        raise HarnessError(f"{lane} nomo build --release capability is unavailable")
    workload_id = workload["id"]
    sources = workload["sources"]
    nomo_source = resolve_suite_path(
        suite_root, sources["nomo"]["path"], f"{workload_id} Nomo source"
    )
    nomo_manifest = resolve_suite_path(
        suite_root,
        sources["nomo"]["project_manifest"],
        f"{workload_id} Nomo manifest",
    )
    project = bundle_root / "build" / workload_id / "release" / lane / "project"
    require_absent_build_path(project, f"{workload_id} {lane} release project")
    v1.copy_nomo_project(nomo_source, nomo_manifest, project)
    project_provenance = formal_project_provenance(
        project,
        nomo_source,
        nomo_manifest,
        sources["nomo"]["path"],
        sources["nomo"]["project_manifest"],
        lane_state,
    )
    project_name = parse_project_name(nomo_manifest)
    binary = binary_path(project / "build" / "bin", project_name)
    generated_c = project / "build" / "c" / "main.c"
    backend_provenance_path = project / "build" / "release-provenance.json"
    require_absent_build_path(
        binary, f"{workload_id} {lane} release binary"
    )
    require_absent_build_path(
        generated_c, f"{workload_id} {lane} release generated C"
    )
    require_absent_build_path(
        backend_provenance_path,
        f"{workload_id} {lane} release backend provenance",
    )
    command = [lane_state["nomo_path"], "build", str(project), "--release"]
    try:
        record, stdout, stderr = run_build_capture(
            command,
            timeout_seconds=build_timeout_seconds,
            cwd=Path(lane_state["checkout"]),
        )
    except BuildCollectionError as error:
        raise WorkloadBuildError(
            str(error),
            workload_build_failure_record(
                workload_id,
                lane,
                "nomo-release-binary",
                nomo_source,
                binary,
                error.record,
            ),
        ) from error
    if "--emit-c" in command or "--release" not in command:
        raise HarnessError(f"{lane} did not use the real release command")
    for output, phase, label in (
        (binary, "nomo-release-binary", "release binary"),
        (generated_c, "nomo-release-generated-c", "release generated C"),
        (
            backend_provenance_path,
            "nomo-release-provenance",
            "release backend provenance",
        ),
    ):
        require_regular_workload_build_output(
            workload_id=workload_id,
            lane=lane,
            phase=phase,
            source=nomo_source,
            output=output,
            command=record,
            stdout=stdout,
            stderr=stderr,
        )
        require_new_build_file(output, f"{workload_id} {lane} {label}")
    backend = read_json_strict(backend_provenance_path)
    validate_release_backend_provenance(
        backend,
        generated_c.resolve(),
        binary.resolve(),
        workload,
        lane,
        toolchains["clang"],
        Path(lane_state["checkout"]),
    )
    return {
        "kind": "real-nomo-release",
        "lane": lane,
        "repository": lane_state["repository"],
        "nomo": {
            "path": lane_state["nomo_path"],
            "sha256": lane_state["nomo_sha256"],
        },
        "source": {"path": str(nomo_source), "sha256": v1.sha256_file(nomo_source)},
        "project": project_provenance,
        "command": record,
        "stdout": stdout.decode("utf-8", errors="replace"),
        "stderr": stderr.decode("utf-8", errors="replace"),
        "generated_c": {
            "path": str(generated_c.resolve()),
            "sha256": v1.sha256_file(generated_c),
            "unmodified_after_build": True,
        },
        "binary": {
            "path": str(binary.resolve()),
            "sha256": v1.sha256_file(binary),
        },
        "backend_provenance_path": str(backend_provenance_path.resolve()),
        "backend_provenance_sha256": v1.sha256_file(backend_provenance_path),
        "backend_provenance": backend,
        "compile_time_excluded_from_run_time": True,
        "emit_c_fallback_used": False,
    }, binary.resolve()


def validate_release_backend_provenance(
    backend: Dict[str, Any],
    generated_c: Path,
    binary: Path,
    workload: Dict[str, Any],
    lane: str,
    clang: Dict[str, Any],
    expected_cwd: Path,
) -> None:
    workload_id = workload["id"]
    label = f"{workload_id} {lane} release backend"
    if backend.get("schema") != 1 or backend.get("complete_argv") is not True:
        raise HarnessError(f"{label} lacks complete machine-readable argv provenance")
    compiler = backend.get("compiler", {})
    required_compiler = {
        "path",
        "realpath",
        "sha256",
        "version_output",
        "target_triple",
    }
    if set(compiler) != required_compiler:
        raise HarnessError(f"{label} compiler identity is incomplete")
    compiler_path = Path(str(compiler["path"]))
    expected_compiler = {
        "path": clang["path"],
        "realpath": clang["realpath"],
        "sha256": clang["sha256"],
        "version_output": clang["version_output"],
        "target_triple": clang["target_triple"],
    }
    if (
        not compiler_path.is_absolute()
        or compiler != expected_compiler
    ):
        raise HarnessError(f"{label} compiler identity is not canonical")
    generated = backend.get("generated_c", {})
    final_binary = backend.get("binary", {})
    if generated != {
        "path": str(generated_c),
        "sha256": v1.sha256_file(generated_c),
    }:
        raise HarnessError(f"{label} generated-C binding is invalid")
    if final_binary != {
        "path": str(binary),
        "sha256": v1.sha256_file(binary),
    }:
        raise HarnessError(f"{label} final-binary binding is invalid")
    objects = backend.get("objects")
    compile_commands = backend.get("compile_commands")
    link_command = backend.get("link_command")
    if (
        not isinstance(objects, list)
        or len(objects) != 1
        or not isinstance(compile_commands, list)
        or len(compile_commands) != 1
    ):
        raise HarnessError(f"{label} compile command list is missing")
    if not isinstance(link_command, dict):
        raise HarnessError(f"{label} link command is missing")
    object_path = objects[0].get("path")
    if (
        not isinstance(object_path, str)
        or not Path(object_path).is_absolute()
        or re.fullmatch(r"[0-9a-f]{64}", str(objects[0].get("sha256"))) is None
    ):
        raise HarnessError(f"{label} object identity is incomplete")
    expected_compile = [
        compiler["path"],
        *CLANG_DRIVER_CONFIG_FLAGS,
        *BASE_C_FLAGS,
        "-c",
        str(generated_c),
        "-o",
        object_path,
    ]
    expected_link = [
        compiler["path"],
        *CLANG_DRIVER_CONFIG_FLAGS,
        object_path,
        "-o",
        str(binary),
        *math_flags(workload),
    ]
    if compile_commands[0].get("argv") != expected_compile:
        raise HarnessError(f"{label} compile argv changed")
    if link_command.get("argv") != expected_link:
        raise HarnessError(f"{label} link argv changed")
    for index, command in enumerate([*compile_commands, link_command]):
        validate_command_record(command, f"{label} command {index}")
        if command.get("cwd") != str(expected_cwd):
            raise HarnessError(f"{label} command {index} cwd changed")
        validate_build_command_environment(
            command, f"{label} command {index}"
        )
        argv = command["argv"]
        validate_clang_driver_config_argv(argv, f"{label} command {index}")
        reject_forbidden_compiler_flags(argv, f"{label} command {index}")
        if argv[0] != compiler["path"]:
            raise HarnessError(f"{label} command switched backend compiler")


def build_emit_c_lane(
    workload: Dict[str, Any],
    suite_root: Path,
    bundle_root: Path,
    lane: str,
    lane_state: Dict[str, Any],
    toolchains: Dict[str, Any],
    build_timeout_seconds: float,
) -> Tuple[Dict[str, Any], Path]:
    if lane_state.get("status") != "available":
        raise HarnessError(f"{lane} compiler lane is unavailable")
    if (
        lane_state.get("capabilities", {})
        .get("emit-c", {})
        .get("status")
        != "available"
    ):
        raise HarnessError(f"{lane} nomo build --emit-c capability is unavailable")
    workload_id = workload["id"]
    sources = workload["sources"]
    nomo_source = resolve_suite_path(
        suite_root, sources["nomo"]["path"], f"{workload_id} Nomo source"
    )
    nomo_manifest = resolve_suite_path(
        suite_root,
        sources["nomo"]["project_manifest"],
        f"{workload_id} Nomo manifest",
    )
    project = bundle_root / "build" / workload_id / "emit-c" / lane / "project"
    require_absent_build_path(project, f"{workload_id} {lane} emit-C project")
    v1.copy_nomo_project(nomo_source, nomo_manifest, project)
    project_provenance = formal_project_provenance(
        project,
        nomo_source,
        nomo_manifest,
        sources["nomo"]["path"],
        sources["nomo"]["project_manifest"],
        lane_state,
    )
    generated_c = project / "build" / "c" / "main.c"
    require_absent_build_path(
        generated_c, f"{workload_id} {lane} emit-C generated C"
    )
    emit_command = [lane_state["nomo_path"], "build", str(project), "--emit-c"]
    try:
        emit_record, emit_stdout, emit_stderr = run_build_capture(
            emit_command,
            timeout_seconds=build_timeout_seconds,
            cwd=Path(lane_state["checkout"]),
        )
    except BuildCollectionError as error:
        raise WorkloadBuildError(
            str(error),
            workload_build_failure_record(
                workload_id,
                lane,
                "nomo-emit-c",
                nomo_source,
                generated_c,
                error.record,
            ),
        ) from error
    require_regular_workload_build_output(
        workload_id=workload_id,
        lane=lane,
        phase="nomo-emit-c",
        source=nomo_source,
        output=generated_c,
        command=emit_record,
        stdout=emit_stdout,
        stderr=emit_stderr,
    )
    require_new_build_file(
        generated_c, f"{workload_id} {lane} emit-C generated C"
    )
    generated_sha_before = v1.sha256_file(generated_c)
    project_name = parse_project_name(nomo_manifest)
    binary = binary_path(project / "build" / "bin", project_name)
    require_absent_build_path(binary, f"{workload_id} {lane} emit-C binary")
    binary.parent.mkdir(parents=True, exist_ok=True)
    clang_command = [
        toolchains["clang"]["path"],
        *CLANG_DRIVER_CONFIG_FLAGS,
        *BASE_C_FLAGS,
        str(generated_c),
        "-o",
        str(binary),
        *math_flags(workload),
    ]
    try:
        clang_record, clang_stdout, clang_stderr = run_build_capture(
            clang_command,
            timeout_seconds=build_timeout_seconds,
            cwd=Path(lane_state["checkout"]),
        )
    except BuildCollectionError as error:
        raise WorkloadBuildError(
            str(error),
            workload_build_failure_record(
                workload_id,
                lane,
                "nomo-generated-c-clang",
                generated_c,
                binary,
                error.record,
            ),
        ) from error
    require_regular_workload_build_output(
        workload_id=workload_id,
        lane=lane,
        phase="nomo-generated-c-clang",
        source=generated_c,
        output=binary,
        command=clang_record,
        stdout=clang_stdout,
        stderr=clang_stderr,
    )
    require_new_build_file(binary, f"{workload_id} {lane} emit-C binary")
    generated_sha_after = v1.sha256_file(generated_c)
    if generated_sha_after != generated_sha_before:
        raise HarnessError(f"{lane} generated C changed after emission")
    return {
        "kind": "nomo-emit-c-clang",
        "lane": lane,
        "repository": lane_state["repository"],
        "nomo": {
            "path": lane_state["nomo_path"],
            "sha256": lane_state["nomo_sha256"],
        },
        "source": {"path": str(nomo_source), "sha256": v1.sha256_file(nomo_source)},
        "project": project_provenance,
        "emit_command": emit_record,
        "emit_stdout": emit_stdout.decode("utf-8", errors="replace"),
        "emit_stderr": emit_stderr.decode("utf-8", errors="replace"),
        "clang_command": clang_record,
        "clang_stdout": clang_stdout.decode("utf-8", errors="replace"),
        "clang_stderr": clang_stderr.decode("utf-8", errors="replace"),
        "generated_c": {
            "path": str(generated_c.resolve()),
            "sha256": generated_sha_before,
            "unmodified_after_emit": True,
        },
        "binary": {
            "path": str(binary.resolve()),
            "sha256": v1.sha256_file(binary),
        },
        "compile_time_excluded_from_run_time": True,
        "release_artifact_reused": False,
    }, binary.resolve()


def correctness_gate(
    manifest: Dict[str, Any],
    suite_root: Path,
    binaries_by_workload: Dict[str, Dict[str, Path]],
    lanes: Sequence[str],
    collector: ProcessCollector,
    build_mode: str,
) -> list[Dict[str, Any]]:
    if build_mode not in (*FORMAL_BUILD_MODES, "baseline-emit-c"):
        raise HarnessError(f"unknown correctness build mode: {build_mode}")
    timeout = float(manifest["methodology"]["correctness_timeout_seconds"])
    results = []
    for workload in manifest["workloads"]:
        fixture = resolve_suite_path(
            suite_root,
            workload["fixtures"]["correctness"]["path"],
            f"{workload['id']} correctness fixture",
        )
        expected = fixture.read_bytes()
        implementations = {}
        attempted_lanes = []
        failure_reason = None
        for lane in lanes:
            environment = {"GOMAXPROCS": "1"} if lane == "go" else {}
            executable = binaries_by_workload[workload["id"]][lane].resolve()
            command = [str(executable), workload["correctness_input"]]
            executable_sha256 = v1.sha256_file(executable)
            attempted_lanes.append(lane)
            try:
                sample, stdout = collector.run(
                    command,
                    expected_stdout=expected,
                    timeout_seconds=timeout,
                    environment_overrides=environment,
                )
            except SampleCollectionError as error:
                sample = error.record
                sample["executable_sha256"] = executable_sha256
                failure_reason = sample["failure_message"]
                implementations[lane] = {
                    "passed": False,
                    "stdout_normalized_sha256": sample[
                        "stdout_normalized_sha256"
                    ],
                    "sample": sample,
                }
                break
            except Exception as error:
                failure_reason = (
                    "correctness collector failed before producing a sample: "
                    f"{type(error).__name__}: {error}"
                )
                sample = _failure_sample(
                    command,
                    _environment(environment)[1],
                    collector.collector_id,
                    dt.datetime.now(dt.timezone.utc).isoformat(),
                    1,
                    b"",
                    str(error).encode("utf-8", errors="replace"),
                    exit_code=None,
                    timed_out=False,
                    failure_kind="collector-failure",
                    message=failure_reason,
                    failure_source="correctness-gate",
                )
                sample["executable_sha256"] = executable_sha256
                implementations[lane] = {
                    "passed": False,
                    "stdout_normalized_sha256": sample[
                        "stdout_normalized_sha256"
                    ],
                    "sample": sample,
                }
                break
            sample["executable_sha256"] = executable_sha256
            implementations[lane] = {
                "passed": True,
                "stdout_normalized_sha256": v1.sha256_bytes(stdout),
                "sample": sample,
            }
        results.append(
            {
                "id": workload["id"],
                "build_mode": build_mode,
                "input": workload["correctness_input"],
                "fixture_path": str(fixture),
                "fixture_sha256": v1.sha256_file(fixture),
                "lanes": list(lanes),
                "attempted_lanes": attempted_lanes,
                "status": (
                    "failed" if failure_reason is not None else "completed"
                ),
                "failure_reason": failure_reason,
                "implementations": implementations,
            }
        )
        if failure_reason is not None:
            break
    return results


def measure_workload_batch(
    manifest: Dict[str, Any],
    suite_root: Path,
    workload: Dict[str, Any],
    binaries: Dict[str, Path],
    collector: ProcessCollector,
    build_mode: str,
    batch_index: int,
    attempt_index: int,
    evidence: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    if build_mode not in FORMAL_BUILD_MODES:
        raise HarnessError(f"unknown formal build mode: {build_mode}")
    schedule = williams_schedule(TIMED_LANES, 30)
    fixture = resolve_suite_path(
        suite_root,
        workload["fixtures"]["performance"]["path"],
        f"{workload['id']} formal fixture",
    )
    expected = fixture.read_bytes()
    timeout = float(manifest["methodology"]["hard_timeout_seconds"])
    record = evidence if evidence is not None else {}
    record.update(
        {
            "id": workload["id"],
            "build_mode": build_mode,
            "performance_input": workload["performance_input"],
            "fixture_path": str(fixture),
            "fixture_sha256": v1.sha256_file(fixture),
            "warmup_orders": schedule[:2],
            "warmups": {lane: [] for lane in TIMED_LANES},
            "block_schedule": [],
            "samples": {lane: [] for lane in TIMED_LANES},
            "collection_status": "running",
            "failure_reason": None,
        }
    )
    warmups = record["warmups"]
    samples = record["samples"]
    warmup_orders = schedule[:2]
    try:
        for warmup_index, order in enumerate(warmup_orders, start=1):
            for position, lane in enumerate(order, start=1):
                environment = {"GOMAXPROCS": "1"} if lane == "go" else {}
                provenance = {
                    "phase": "warmup",
                    "build_mode": build_mode,
                    "batch_index": batch_index,
                    "attempt_index": attempt_index,
                    "warmup_index": warmup_index,
                    "order_position": position,
                    "executable_sha256": v1.sha256_file(binaries[lane]),
                }
                try:
                    sample, _ = collector.run(
                        [str(binaries[lane]), workload["performance_input"]],
                        expected_stdout=expected,
                        timeout_seconds=timeout,
                        environment_overrides=environment,
                    )
                except SampleCollectionError as error:
                    error.record.update(provenance)
                    warmups[lane].append(error.record)
                    raise
                except Exception as error:
                    message = (
                        f"collector failed before producing a sample: "
                        f"{type(error).__name__}: {error}"
                    )
                    failed = _failure_sample(
                        [str(binaries[lane]), workload["performance_input"]],
                        _environment(environment)[1],
                        collector.collector_id,
                        dt.datetime.now(dt.timezone.utc).isoformat(),
                        1,
                        b"",
                        str(error).encode("utf-8", errors="replace"),
                        exit_code=None,
                        timed_out=False,
                        failure_kind="collector-failure",
                        message=message,
                        failure_source="measurement-harness",
                    )
                    failed.update(provenance)
                    warmups[lane].append(failed)
                    raise SampleCollectionError(message, failed) from error
                sample.update(provenance)
                warmups[lane].append(sample)
        for block_index, order in enumerate(schedule, start=1):
            block_record = {"block_index": block_index, "order": list(order)}
            record["block_schedule"].append(block_record)
            for position, lane in enumerate(order, start=1):
                environment = {"GOMAXPROCS": "1"} if lane == "go" else {}
                provenance = {
                    "phase": "sample",
                    "build_mode": build_mode,
                    "batch_index": batch_index,
                    "attempt_index": attempt_index,
                    "block_index": block_index,
                    "order_position": position,
                    "executable_sha256": v1.sha256_file(binaries[lane]),
                }
                try:
                    sample, _ = collector.run(
                        [str(binaries[lane]), workload["performance_input"]],
                        expected_stdout=expected,
                        timeout_seconds=timeout,
                        environment_overrides=environment,
                    )
                except SampleCollectionError as error:
                    error.record.update(provenance)
                    samples[lane].append(error.record)
                    raise
                except Exception as error:
                    message = (
                        f"collector failed before producing a sample: "
                        f"{type(error).__name__}: {error}"
                    )
                    failed = _failure_sample(
                        [str(binaries[lane]), workload["performance_input"]],
                        _environment(environment)[1],
                        collector.collector_id,
                        dt.datetime.now(dt.timezone.utc).isoformat(),
                        1,
                        b"",
                        str(error).encode("utf-8", errors="replace"),
                        exit_code=None,
                        timed_out=False,
                        failure_kind="collector-failure",
                        message=message,
                        failure_source="measurement-harness",
                    )
                    failed.update(provenance)
                    samples[lane].append(failed)
                    raise SampleCollectionError(message, failed) from error
                sample.update(provenance)
                samples[lane].append(sample)
    except HarnessError as error:
        record["collection_status"] = "failed"
        record["failure_reason"] = str(error)
        raise
    record["collection_status"] = "completed"
    return record


def collect_protocol_batches(
    manifest: Dict[str, Any],
    suite_root: Path,
    binaries_by_workload: Dict[str, Dict[str, Path]],
    collector: ProcessCollector,
    build_mode: str,
    static_authorization: Dict[str, Any],
    measure_function: Callable[..., Dict[str, Any]] = measure_workload_batch,
    dynamic_snapshot_function: Callable[[str], Dict[str, Any]] = capture_dynamic_environment,
) -> list[Dict[str, Any]]:
    accepted_batches = 0
    attempt_index = 0
    reruns_used = 0
    batches: list[Dict[str, Any]] = []
    maximum_reruns = int(
        manifest["methodology"]["batch_invalidation"]["maximum_reruns"]
    )
    while accepted_batches < 2 and attempt_index < 2 + maximum_reruns:
        batch_index = accepted_batches + 1
        attempt_index += 1
        batch_started_at_utc = dt.datetime.now(dt.timezone.utc).isoformat()
        dynamic_environment_before = dynamic_snapshot_function(
            static_authorization["expected_bindings"][
                "authority_host_sha256"
            ],
            static_authorization["dynamic_policy"],
        )
        batch: Dict[str, Any] = {
            "build_mode": build_mode,
            "batch_index": batch_index,
            "attempt_index": attempt_index,
            "started_at_utc": batch_started_at_utc,
            "static_authorization_sha256": static_authorization.get(
                "qualification_sha256"
            ),
            "dynamic_environment_before": dynamic_environment_before,
            "workloads": [],
            "status": "running",
            "invalidation_reasons": [],
            "stability": None,
            "evaluation": None,
        }
        try:
            if not batch["dynamic_environment_before"]["eligible"]:
                raise HarnessError(
                    "dynamic environment was ineligible before the batch"
                )
            for workload in manifest["workloads"]:
                partial: Dict[str, Any] = {
                    "id": workload["id"],
                    "build_mode": build_mode,
                    "collection_status": "not-started",
                    "failure_reason": None,
                    "warmups": {lane: [] for lane in TIMED_LANES},
                    "samples": {lane: [] for lane in TIMED_LANES},
                    "block_schedule": [],
                }
                batch["workloads"].append(partial)
                measured = measure_function(
                    manifest,
                    suite_root,
                    workload,
                    binaries_by_workload[workload["id"]],
                    collector,
                    build_mode,
                    batch_index,
                    attempt_index,
                    partial,
                )
                if measured is not partial:
                    partial.clear()
                    partial.update(measured)
            batch["stability"] = batch_stability(
                batch["workloads"],
                manifest["methodology"]["batch_invalidation"],
            )
            if batch["stability"]["valid"]:
                batch["evaluation"] = evaluate_batch(
                    batch["workloads"], manifest["thresholds"], True
                )
                batch["status"] = "completed"
                accepted_batches += 1
            else:
                batch["status"] = "invalidated"
                batch["invalidation_reasons"].extend(
                    batch["stability"]["issues"]
                )
                batch["evaluation"] = {
                    "environment_eligible": False,
                    "verdict": "ineligible",
                }
        except HarnessError as error:
            batch["status"] = "invalidated"
            batch["invalidation_reasons"].append(str(error))
            batch["evaluation"] = {
                "environment_eligible": False,
                "verdict": "ineligible",
            }
        batch["dynamic_environment_after"] = dynamic_snapshot_function(
            static_authorization["expected_bindings"]["authority_host_sha256"],
            static_authorization["dynamic_policy"],
        )
        dynamic_issues = dynamic_environment_pair_issues(
            batch["dynamic_environment_before"],
            batch["dynamic_environment_after"],
            static_authorization["dynamic_policy"],
        )
        if dynamic_issues:
            if batch["status"] == "completed":
                accepted_batches -= 1
            batch["status"] = "invalidated"
            batch["invalidation_reasons"].extend(dynamic_issues)
            batch["evaluation"] = {
                "environment_eligible": False,
                "verdict": "ineligible",
            }
        batch["finished_at_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
        batches.append(batch)
        if batch["status"] != "completed":
            if reruns_used >= maximum_reruns:
                break
            reruns_used += 1
    return batches


def dynamic_environment_pair_issues(
    before: Dict[str, Any],
    after: Dict[str, Any],
    policy: Dict[str, Any],
) -> list[str]:
    if policy != DYNAMIC_ENVIRONMENT_POLICY:
        return ["dynamic environment policy changed"]
    issues = []
    if not before.get("eligible") or not after.get("eligible"):
        issues.append("dynamic environment snapshot was ineligible")
    if before.get("snapshot_sha256") == after.get("snapshot_sha256"):
        issues.append("dynamic environment snapshot was reused")
    if int(after.get("monotonic_ns", -1)) <= int(before.get("monotonic_ns", -1)):
        issues.append("dynamic environment monotonic order is invalid")
    before_observations = before.get("observations", {})
    after_observations = after.get("observations", {})
    def parsed(
        observations: Dict[str, Any], observation_id: str
    ) -> Dict[str, Any]:
        value = observations.get(observation_id, {}).get("parsed")
        return value if isinstance(value, dict) else {}

    before_power = parsed(before_observations, "power_mode")
    after_power = parsed(after_observations, "power_mode")
    if policy["require_ac_power"] and (
        before_power.get("ac_power") is not True
        or after_power.get("ac_power") is not True
    ):
        issues.append("AC power was not stable for the complete batch")
    before_low = parsed(before_observations, "low_power_mode")
    after_low = parsed(after_observations, "low_power_mode")
    if not policy["allow_low_power_mode"] and (
        before_low.get("enabled") is not False
        or after_low.get("enabled") is not False
    ):
        issues.append("low-power mode was enabled or unparseable")
    for observation_id in ("frequency_governor", "affinity"):
        if parsed(before_observations, observation_id) != parsed(
            after_observations, observation_id
        ):
            issues.append(f"{observation_id} changed during the batch")
    for snapshot_name, observations in (
        ("before", before_observations),
        ("after", after_observations),
    ):
        concurrent_load = parsed(
            observations, "concurrent_load"
        ).get("one_minute_per_logical_core")
        if (
            not isinstance(concurrent_load, (int, float))
            or not math.isfinite(float(concurrent_load))
            or concurrent_load < 0
            or concurrent_load >= policy["max_load_per_logical_core"]
        ):
            issues.append(
                f"{snapshot_name} concurrent load was unparseable or exceeded threshold"
            )
        thermal = parsed(observations, "thermal_state")
        if thermal.get("normal") is False:
            issues.append(f"{snapshot_name} thermal state was abnormal")
        maximum = thermal.get("maximum_celsius")
        if (
            isinstance(maximum, (int, float))
            and maximum > policy["max_thermal_celsius"]
        ):
            issues.append(f"{snapshot_name} thermal threshold was exceeded")
    before_swap = parsed(before_observations, "swap").get("used_bytes")
    after_swap = parsed(after_observations, "swap").get("used_bytes")
    if not isinstance(before_swap, int) or not isinstance(after_swap, int):
        issues.append("swap usage could not be compared")
    elif after_swap - before_swap > policy["max_swap_delta_bytes"]:
        issues.append("swap usage increased during the batch")
    return issues


def protocol_result(
    manifest: Dict[str, Any],
    suite_root: Path,
    binaries_by_workload: Dict[str, Dict[str, Path]],
    collector: ProcessCollector,
    build_mode: str,
    static_authorization: Dict[str, Any],
) -> Dict[str, Any]:
    correctness = correctness_gate(
        manifest,
        suite_root,
        binaries_by_workload,
        CORRECTNESS_FORMAL_LANES,
        collector,
        build_mode,
    )
    if (
        len(correctness) != len(WORKLOAD_IDS)
        or any(item.get("status") != "completed" for item in correctness)
    ):
        failed = correctness[-1]
        return {
            "build_mode": build_mode,
            "status": "ineligible",
            "reason": (
                "formal correctness gate failed: "
                + str(failed.get("failure_reason"))
            ),
            "correctness": correctness,
            "batches": [],
            "verdict": "not_evaluated",
        }
    batches = collect_protocol_batches(
        manifest,
        suite_root,
        binaries_by_workload,
        collector,
        build_mode,
        static_authorization,
    )
    completed = [batch for batch in batches if batch["status"] == "completed"]
    if len(completed) != 2:
        return {
            "build_mode": build_mode,
            "status": "ineligible",
            "reason": "two independent stable batches were not completed",
            "correctness": correctness,
            "batches": batches,
            "verdict": "not_evaluated",
        }
    verdict = (
        "pass"
        if all(batch["evaluation"]["verdict"] == "pass" for batch in completed)
        else "fail"
    )
    return {
        "build_mode": build_mode,
        "status": "completed",
        "reason": "two independent stable batches completed",
        "correctness": correctness,
        "batches": batches,
        "verdict": verdict,
    }


def aggregate_protocol_outcome(
    protocols: Dict[str, Dict[str, Any]],
) -> Dict[str, Any]:
    statuses = [
        protocols[build_mode].get("status")
        for build_mode in FORMAL_BUILD_MODES
    ]
    if all(status == "completed" for status in statuses):
        overall = (
            "pass"
            if all(
                protocols[build_mode].get("verdict") == "pass"
                for build_mode in FORMAL_BUILD_MODES
            )
            else "fail"
        )
        return {
            "status": "completed",
            "overall_verdict": overall,
            "claim_eligible": overall == "pass",
        }
    if "ineligible" in statuses:
        return {
            "status": "ineligible",
            "overall_verdict": "not_evaluated",
            "claim_eligible": False,
        }
    return {
        "status": "unavailable",
        "overall_verdict": "not_evaluated",
        "claim_eligible": False,
    }


def artifact_path(value: Any, host_os: str) -> PurePath:
    path_type = PureWindowsPath if host_os == "Windows" else PurePosixPath
    return path_type(str(value))


def artifact_path_is_absolute(value: Any, host_os: str) -> bool:
    return artifact_path(value, host_os).is_absolute()


def artifact_paths_overlap(
    left: Any, right: Any, host_os: str
) -> bool:
    left_path = artifact_path(left, host_os)
    right_path = artifact_path(right, host_os)
    if host_os == "Windows":
        left_parts = tuple(part.casefold() for part in left_path.parts)
        right_parts = tuple(part.casefold() for part in right_path.parts)
    else:
        left_parts = left_path.parts
        right_parts = right_path.parts
    shorter = min(len(left_parts), len(right_parts))
    return left_parts[:shorter] == right_parts[:shorter]


def artifact_file_identity_equal(
    left: Any,
    right: Any,
    host_os: str,
    *,
    inspect_live_filesystem: bool = True,
) -> bool:
    left_text = str(left)
    right_text = str(right)
    if artifact_paths_overlap(left_text, right_text, host_os):
        left_path = artifact_path(left_text, host_os)
        right_path = artifact_path(right_text, host_os)
        if len(left_path.parts) == len(right_path.parts):
            return True
    if inspect_live_filesystem and platform.system() == host_os:
        left_live = Path(left_text)
        right_live = Path(right_text)
        if left_live.exists() and right_live.exists():
            try:
                return os.path.samefile(left_live, right_live)
            except OSError:
                return False
    return False


def validate_formal_project_record(
    formal: Dict[str, Any],
    workload: Dict[str, Any],
    lane: str,
    lane_state: Dict[str, Any],
    recorded_suite_root: PurePath,
    recorded_host_os: str,
) -> PurePath:
    workload_id = workload["id"]
    project = formal.get("project", {})
    project_path_value = project.get("path")
    if not artifact_path_is_absolute(
        project_path_value, recorded_host_os
    ):
        raise HarnessError(
            f"{workload_id} {lane} formal project path is not absolute"
        )
    project_path = artifact_path(project_path_value, recorded_host_os)
    nomo_source = workload["sources"]["nomo"]
    source_relative = nomo_source["path"]
    manifest_relative = nomo_source["project_manifest"]
    expected_source = {
        "path": str(recorded_suite_root / source_relative),
        "sha256": nomo_source["sha256"],
    }
    expected_manifest = {
        "path": str(recorded_suite_root / manifest_relative),
        "sha256": nomo_source["project_manifest_sha256"],
    }
    manifest_relative_path = PurePosixPath(manifest_relative)
    source_project_relative = PurePosixPath(source_relative).relative_to(
        manifest_relative_path.parent
    )
    copied_source = {
        "path": str(project_path / str(source_project_relative)),
        "sha256": nomo_source["sha256"],
    }
    copied_manifest = {
        "path": str(project_path / manifest_relative_path.name),
        "sha256": nomo_source["project_manifest_sha256"],
    }
    if (
        project.get("source_relative_path") != source_relative
        or project.get("source") != expected_source
        or formal.get("source") != expected_source
        or project.get("project_manifest_relative_path")
        != manifest_relative
        or project.get("project_manifest") != expected_manifest
        or project.get("copied_source") != copied_source
        or project.get("copied_project_manifest") != copied_manifest
        or project.get("compiler_checkout")
        != lane_state.get("checkout")
        or project.get("compiler_commit")
        != lane_state.get("expected_commit")
    ):
        raise HarnessError(
            f"{workload_id} {lane} formal project input is not "
            "manifest/checkpoint-bound"
        )
    return project_path


def validate_formal_slot_record(
    formal: Dict[str, Any],
    workload_id: str,
    build_mode: str,
    lane: str,
    lane_state: Dict[str, Any],
) -> None:
    expected_nomo = {
        "path": lane_state.get("nomo_path"),
        "sha256": lane_state.get("nomo_sha256"),
    }
    if (
        formal.get("lane") != lane
        or formal.get("repository") != lane_state.get("repository")
        or formal.get("nomo") != expected_nomo
    ):
        raise HarnessError(
            f"{workload_id} {build_mode} {lane} formal slot is not "
            "exactly bound to its compiler lane authority"
        )


def validate_build_provenance(
    result: Dict[str, Any],
    manifest: Dict[str, Any],
    *,
    live_filesystem: bool = True,
) -> None:
    provenance = result.get("provenance", {})
    toolchains = provenance.get("toolchains", {})
    recorded_host_os = str(
        provenance.get("host", {}).get("os", "")
    )
    build_environment = toolchains.get("build_environment")
    if not isinstance(build_environment, dict):
        raise HarnessError("recorded build environment authority is missing")
    manifest_path = provenance.get("manifest_path")
    if not isinstance(manifest_path, str) or not artifact_path_is_absolute(
        manifest_path, recorded_host_os
    ):
        raise HarnessError("recorded manifest path is not absolute")
    recorded_suite_root = artifact_path(
        manifest_path, recorded_host_os
    ).parent
    workloads = {workload["id"]: workload for workload in manifest["workloads"]}
    for workload_id, build in result.get("builds", {}).items():
        if workload_id not in workloads:
            raise HarnessError(f"unknown workload build provenance: {workload_id}")
        workload = workloads[workload_id]
        link_flags = math_flags_for_host(workload, recorded_host_os)
        references = build.get("references")
        if references is None:
            continue
        commands = references.get("commands", {})
        source_files = references.get("source_files", {})
        binaries = references.get("binaries", {})
        generated_c = references.get("generated_c")
        baseline_present = (
            isinstance(generated_c, dict)
            or "nomo" in source_files
            or "nomo-baseline" in binaries
            or any(name in commands for name in NOMO_BASELINE_BUILD_COMMANDS)
        )
        baseline_required = baseline_present or result.get("mode") == "correctness"
        expected_reference_lanes = set(REFERENCE_LANES)
        expected_source_names = set(expected_reference_lanes)
        expected_binary_names = set(expected_reference_lanes)
        expected_output_names = set(expected_reference_lanes)
        if baseline_required:
            expected_source_names.add("nomo")
            expected_binary_names.add("nomo-baseline")
            expected_output_names.add("nomo-baseline")
        compiled_sources = references.get("compiled_sources", {})
        compiler_output = references.get("compiler_output", {})
        if (
            set(source_files) != expected_source_names
            or set(compiled_sources) != expected_reference_lanes
            or set(binaries) != expected_binary_names
            or set(compiler_output) != expected_output_names
        ):
            raise HarnessError(
                f"{workload_id} reference provenance must contain the exact "
                "frozen source, compiled-source, output, and binary key sets"
            )
        for lane in REFERENCE_LANES:
            output = compiler_output.get(lane)
            if (
                not isinstance(output, dict)
                or set(output) != {"stdout", "stderr"}
                or not all(isinstance(value, str) for value in output.values())
            ):
                raise HarnessError(
                    f"{workload_id} {lane} compiler output evidence is incomplete"
                )
        if baseline_required:
            baseline_output = compiler_output.get("nomo-baseline")
            if (
                not isinstance(baseline_output, dict)
                or set(baseline_output)
                != {
                    "emit_stdout",
                    "emit_stderr",
                    "clang_stdout",
                    "clang_stderr",
                }
                or not all(
                    isinstance(value, str)
                    for value in baseline_output.values()
                )
            ):
                raise HarnessError(
                    f"{workload_id} Nomo baseline compiler output is incomplete"
                )
        expected_command_names = set(REFERENCE_BUILD_COMMANDS)
        if baseline_required:
            expected_command_names.update(NOMO_BASELINE_BUILD_COMMANDS)
        if set(commands) != expected_command_names:
            raise HarnessError(
                f"{workload_id} reference build must contain the exact frozen "
                f"command set: {', '.join(sorted(expected_command_names))}"
            )
        go_source_path = artifact_path(
            compiled_sources.get("go", {}).get("path", ""),
            recorded_host_os,
        )
        go_cache_environment = {
            key: str(go_source_path.parent / directory)
            for key, directory in GO_BUILD_CACHE_DIRECTORIES.items()
        }
        for name, record in commands.items():
            validate_command_record(record, f"{workload_id} {name}")
            expected_reference_cwd = str(
                recorded_suite_root.parent.parent
            )
            if record.get("cwd") != expected_reference_cwd:
                raise HarnessError(
                    f"{workload_id} {name} cwd is not the canonical repository root"
                )
            validate_build_command_environment(
                record,
                f"{workload_id} {name}",
                go_cache_environment if name == "go_build" else None,
                build_environment,
            )
            reject_forbidden_compiler_flags(
                record["argv"], f"{workload_id} {name}"
            )
        if live_filesystem and any(
            Path(path).exists() for path in go_cache_environment.values()
        ):
            raise HarnessError(
                f"{workload_id} isolated Go build cache was not removed"
            )
        for lane in REFERENCE_LANES:
            manifest_source = workload["sources"][lane]
            expected_original = {
                "path": str(recorded_suite_root / manifest_source["path"]),
                "sha256": manifest_source["sha256"],
            }
            if source_files.get(lane) != expected_original:
                raise HarnessError(
                    f"{workload_id} {lane} original source is not manifest-bound"
                )
            compiled = compiled_sources.get(lane, {})
            if (
                not artifact_path_is_absolute(
                    compiled.get("path"), recorded_host_os
                )
                or compiled.get("sha256") != manifest_source["sha256"]
            ):
                raise HarnessError(
                    f"{workload_id} {lane} compiled source is not a locked copy"
                )
            binary = binaries.get(lane, {})
            if (
                not artifact_path_is_absolute(
                    binary.get("path"), recorded_host_os
                )
                or re.fullmatch(r"[0-9a-f]{64}", str(binary.get("sha256")))
                is None
            ):
                raise HarnessError(f"{workload_id} {lane} binary identity is invalid")
        exact_reference_commands = {
            "c_build": [
                toolchains.get("clang", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                compiled_sources.get("c", {}).get("path"),
                "-o",
                binaries.get("c", {}).get("path"),
                *link_flags,
            ],
            "cpp_build": [
                toolchains.get("clangxx", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_CPP_FLAGS,
                compiled_sources.get("cpp", {}).get("path"),
                "-o",
                binaries.get("cpp", {}).get("path"),
                *link_flags,
            ],
            "semantic-c_build": [
                toolchains.get("clang", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                compiled_sources.get("semantic-c", {}).get("path"),
                "-o",
                binaries.get("semantic-c", {}).get("path"),
                *link_flags,
            ],
            "go_build": [
                toolchains.get("go", {}).get("path"),
                "build",
                "-o",
                binaries.get("go", {}).get("path"),
                compiled_sources.get("go", {}).get("path"),
            ],
        }
        for name, expected_argv in exact_reference_commands.items():
            if commands[name]["argv"] != expected_argv:
                raise HarnessError(
                    f"{workload_id} {name} does not match the complete frozen argv"
                )
            if name != "go_build":
                validate_clang_driver_config_argv(
                    commands[name]["argv"], f"{workload_id} {name}"
                )
                reject_forbidden_compiler_flags(
                    commands[name]["argv"], f"{workload_id} {name}"
                )
        if baseline_required:
            manifest_nomo = workload["sources"]["nomo"]
            expected_nomo_sources = [
                {
                    "path": str(
                        recorded_suite_root / manifest_nomo["path"]
                    ),
                    "sha256": manifest_nomo["sha256"],
                },
                {
                    "path": str(
                        recorded_suite_root
                        / manifest_nomo["project_manifest"]
                    ),
                    "sha256": manifest_nomo["project_manifest_sha256"],
                },
            ]
            if source_files.get("nomo") != expected_nomo_sources:
                raise HarnessError(
                    f"{workload_id} Nomo baseline sources are not manifest-bound"
                )
            generated_path = (
                generated_c.get("path")
                if isinstance(generated_c, dict)
                else None
            )
            generated_sha = (
                generated_c.get("sha256")
                if isinstance(generated_c, dict)
                else None
            )
            baseline_binary_record = binaries.get("nomo-baseline", {})
            baseline_binary = baseline_binary_record.get("path")
            baseline_binary_sha = baseline_binary_record.get("sha256")
            if (
                not isinstance(generated_path, str)
                or not artifact_path_is_absolute(
                    generated_path, recorded_host_os
                )
                or re.fullmatch(r"[0-9a-f]{64}", str(generated_sha)) is None
                or not isinstance(baseline_binary, str)
                or not artifact_path_is_absolute(
                    baseline_binary, recorded_host_os
                )
                or re.fullmatch(r"[0-9a-f]{64}", str(baseline_binary_sha))
                is None
            ):
                raise HarnessError(
                    f"{workload_id} Nomo baseline output identity is incomplete"
                )
            generated_path_object = artifact_path(
                generated_path, recorded_host_os
            )
            try:
                baseline_project = generated_path_object.parents[2]
            except IndexError as error:
                raise HarnessError(
                    f"{workload_id} Nomo baseline generated-C path is invalid"
                ) from error
            if generated_path_object != (
                baseline_project / "build" / "c" / "main.c"
            ):
                raise HarnessError(
                    f"{workload_id} Nomo baseline generated-C path changed"
                )
            nomo_toolchain = toolchains.get("nomo", {})
            nomo_path = nomo_toolchain.get("path")
            if (
                not isinstance(nomo_path, str)
                or not artifact_path_is_absolute(
                    nomo_path, recorded_host_os
                )
                or re.fullmatch(
                    r"[0-9a-f]{64}", str(nomo_toolchain.get("sha256"))
                )
                is None
            ):
                raise HarnessError(
                    f"{workload_id} Nomo baseline tool identity is incomplete"
                )
            clang_toolchain = toolchains.get("clang", {})
            if re.fullmatch(
                r"[0-9a-f]{64}", str(clang_toolchain.get("sha256"))
            ) is None:
                raise HarnessError(
                    f"{workload_id} baseline Clang tool identity is incomplete"
                )
            expected_emit = [
                nomo_path,
                "build",
                str(baseline_project),
                "--emit-c",
            ]
            if commands["nomo_baseline_emit_c"]["argv"] != expected_emit:
                raise HarnessError(
                    f"{workload_id} baseline emit-C argv changed"
                )
            expected_clang = [
                clang_toolchain.get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                generated_path,
                "-o",
                baseline_binary,
                *link_flags,
            ]
            if commands["nomo_baseline_clang"]["argv"] != expected_clang:
                raise HarnessError(
                    f"{workload_id} baseline generated-C argv changed"
                )
            reject_forbidden_compiler_flags(
                commands["nomo_baseline_clang"]["argv"],
                f"{workload_id} baseline generated-C",
            )
            validate_clang_driver_config_argv(
                commands["nomo_baseline_clang"]["argv"],
                f"{workload_id} baseline generated-C",
            )
        modes = build.get("modes", {})
        formal_projects: list[Tuple[str, str]] = []
        formal_generated: list[Tuple[str, str]] = []
        formal_objects: list[Tuple[str, str]] = []
        all_binaries: list[Tuple[str, str]] = [
            (f"reference {lane}", binaries[lane]["path"])
            for lane in REFERENCE_LANES
        ]
        for lane in ("candidate", "main"):
            lane_state = result.get("release_lanes", {}).get(lane, {})
            release = modes.get("release", {}).get(lane)
            if release is not None:
                validate_formal_slot_record(
                    release,
                    workload_id,
                    "release",
                    lane,
                    lane_state,
                )
                release_project = validate_formal_project_record(
                    release,
                    workload,
                    lane,
                    lane_state,
                    recorded_suite_root,
                    recorded_host_os,
                )
                command = release.get("command", {})
                validate_command_record(command, f"{workload_id} {lane} release")
                validate_build_command_environment(
                    command,
                    f"{workload_id} {lane} release",
                    authority_projection=build_environment,
                )
                argv = command["argv"]
                release_generated = release.get("generated_c", {})
                generated_path = release_generated.get("path")
                binary_path_value = release.get("binary", {}).get("path")
                expected_release = [
                    lane_state_path
                    if (lane_state_path := result.get("release_lanes", {}).get(lane, {}).get("nomo_path"))
                    else None,
                    "build",
                    str(release_project),
                    "--release",
                ]
                if (
                    argv != expected_release
                    or command.get("cwd") != lane_state.get("checkout")
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} release argv changed or cwd changed"
                    )
                if release.get("emit_c_fallback_used") is not False:
                    raise HarnessError("release protocol cannot use emit-c fallback")
                if (
                    set(release_generated)
                    != {"path", "sha256", "unmodified_after_build"}
                    or release_generated.get("unmodified_after_build") is not True
                    or not artifact_path_is_absolute(
                        generated_path, recorded_host_os
                    )
                    or artifact_path(generated_path, recorded_host_os)
                    != release_project / "build" / "c" / "main.c"
                    or not artifact_path_is_absolute(
                        binary_path_value, recorded_host_os
                    )
                    or not artifact_paths_overlap(
                        release_project,
                        binary_path_value,
                        recorded_host_os,
                    )
                    or re.fullmatch(
                        r"[0-9a-f]{64}",
                        str(release.get("generated_c", {}).get("sha256")),
                    )
                    is None
                    or re.fullmatch(
                        r"[0-9a-f]{64}",
                        str(release.get("binary", {}).get("sha256")),
                    )
                    is None
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} release outputs are not "
                        "project-bound"
                    )
                backend = release.get("backend_provenance")
                if not isinstance(backend, dict):
                    raise HarnessError(
                        f"{workload_id} {lane} release backend provenance is missing"
                    )
                expected_compiler = {
                    key: toolchains.get("clang", {}).get(key)
                    for key in (
                        "path",
                        "realpath",
                        "sha256",
                        "version_output",
                        "target_triple",
                    )
                }
                if backend.get("compiler") != expected_compiler:
                    raise HarnessError(
                        f"{workload_id} {lane} release backend compiler changed"
                    )
                objects = backend.get("objects", [])
                compile_commands = backend.get("compile_commands", [])
                if len(objects) != 1 or len(compile_commands) != 1:
                    raise HarnessError(
                        f"{workload_id} {lane} release backend translation units changed"
                    )
                expected_compile = [
                    expected_compiler["path"],
                    *CLANG_DRIVER_CONFIG_FLAGS,
                    *BASE_C_FLAGS,
                    "-c",
                    release["generated_c"]["path"],
                    "-o",
                    objects[0].get("path"),
                ]
                expected_link = [
                    expected_compiler["path"],
                    *CLANG_DRIVER_CONFIG_FLAGS,
                    objects[0].get("path"),
                    "-o",
                    release["binary"]["path"],
                    *link_flags,
                ]
                if compile_commands[0].get("argv") != expected_compile:
                    raise HarnessError(
                        f"{workload_id} {lane} release backend C argv changed"
                    )
                if backend.get("link_command", {}).get("argv") != expected_link:
                    raise HarnessError(
                        f"{workload_id} {lane} release backend link argv changed"
                    )
                if backend.get("generated_c") != {
                    "path": generated_path,
                    "sha256": release["generated_c"]["sha256"],
                } or backend.get("binary") != release.get("binary"):
                    raise HarnessError(
                        f"{workload_id} {lane} release backend outputs differ "
                        "from the outer formal record"
                    )
                backend_path = release.get("backend_provenance_path")
                if (
                    not artifact_path_is_absolute(
                        backend_path, recorded_host_os
                    )
                    or artifact_path(backend_path, recorded_host_os)
                    != release_project / "build" / "release-provenance.json"
                    or re.fullmatch(
                        r"[0-9a-f]{64}",
                        str(release.get("backend_provenance_sha256")),
                    )
                    is None
                    or not artifact_path_is_absolute(
                        objects[0].get("path"), recorded_host_os
                    )
                    or not artifact_paths_overlap(
                        release_project,
                        objects[0].get("path"),
                        recorded_host_os,
                    )
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} release sidecar or object is "
                        "not project-bound"
                    )
                for index, backend_command in enumerate(
                    [
                        *backend.get("compile_commands", []),
                        backend.get("link_command", {}),
                    ]
                ):
                    validate_command_record(
                        backend_command,
                        f"{workload_id} {lane} release backend {index}",
                    )
                    if backend_command.get("cwd") != lane_state.get(
                        "checkout"
                    ):
                        raise HarnessError(
                            f"{workload_id} {lane} release backend {index} "
                            "cwd changed"
                        )
                    validate_build_command_environment(
                        backend_command,
                        f"{workload_id} {lane} release backend {index}",
                        authority_projection=build_environment,
                    )
                    reject_forbidden_compiler_flags(
                        backend_command["argv"],
                        f"{workload_id} {lane} release backend {index}",
                    )
                    validate_clang_driver_config_argv(
                        backend_command["argv"],
                        f"{workload_id} {lane} release backend {index}",
                    )
                formal_projects.append(
                    (f"release {lane}", str(release_project))
                )
                formal_generated.append(
                    (f"release {lane}", str(generated_path))
                )
                formal_objects.append(
                    (f"release {lane}", str(objects[0]["path"]))
                )
                all_binaries.append(
                    (f"release {lane}", str(binary_path_value))
                )
            emit_c = modes.get("emit-c", {}).get(lane)
            if emit_c is not None:
                validate_formal_slot_record(
                    emit_c,
                    workload_id,
                    "emit-c",
                    lane,
                    lane_state,
                )
                emit_project = validate_formal_project_record(
                    emit_c,
                    workload,
                    lane,
                    lane_state,
                    recorded_suite_root,
                    recorded_host_os,
                )
                emit_command = emit_c.get("emit_command", {})
                clang_command = emit_c.get("clang_command", {})
                validate_command_record(
                    emit_command, f"{workload_id} {lane} emit-c"
                )
                validate_build_command_environment(
                    emit_command,
                    f"{workload_id} {lane} emit-c",
                    authority_projection=build_environment,
                )
                validate_command_record(
                    clang_command, f"{workload_id} {lane} generated-C Clang"
                )
                validate_build_command_environment(
                    clang_command,
                    f"{workload_id} {lane} generated-C Clang",
                    authority_projection=build_environment,
                )
                generated_path = emit_c.get("generated_c", {}).get("path")
                binary_path_value = emit_c.get("binary", {}).get("path")
                expected_emit = [
                    result.get("release_lanes", {}).get(lane, {}).get("nomo_path"),
                    "build",
                    str(emit_project),
                    "--emit-c",
                ]
                if (
                    emit_command["argv"] != expected_emit
                    or emit_command.get("cwd") != lane_state.get("checkout")
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} emit-c command or cwd changed"
                    )
                expected_clang = [
                    toolchains.get("clang", {}).get("path"),
                    *CLANG_DRIVER_CONFIG_FLAGS,
                    *BASE_C_FLAGS,
                    generated_path,
                    "-o",
                    binary_path_value,
                    *link_flags,
                ]
                if (
                    clang_command["argv"] != expected_clang
                    or clang_command.get("cwd") != lane_state.get("checkout")
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} generated-C Clang argv changed "
                        "or cwd changed"
                    )
                reject_forbidden_compiler_flags(
                    clang_command["argv"],
                    f"{workload_id} {lane} generated-C Clang",
                )
                validate_clang_driver_config_argv(
                    clang_command["argv"],
                    f"{workload_id} {lane} generated-C Clang",
                )
                if emit_c.get("release_artifact_reused") is not False:
                    raise HarnessError(
                        f"{workload_id} {lane} emit-c must be an independent build"
                    )
                generated = emit_c.get("generated_c", {})
                if (
                    set(generated)
                    != {"path", "sha256", "unmodified_after_emit"}
                    or generated.get("unmodified_after_emit") is not True
                    or not artifact_path_is_absolute(
                        generated_path, recorded_host_os
                    )
                    or artifact_path(generated_path, recorded_host_os)
                    != emit_project / "build" / "c" / "main.c"
                    or not artifact_path_is_absolute(
                        binary_path_value, recorded_host_os
                    )
                    or not artifact_paths_overlap(
                        emit_project,
                        binary_path_value,
                        recorded_host_os,
                    )
                    or re.fullmatch(
                        r"[0-9a-f]{64}", str(generated.get("sha256"))
                    )
                    is None
                    or re.fullmatch(
                        r"[0-9a-f]{64}",
                        str(emit_c.get("binary", {}).get("sha256")),
                    )
                    is None
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} emit-C outputs are not "
                        "independent project-bound files"
                    )
                formal_projects.append(
                    (f"emit-c {lane}", str(emit_project))
                )
                formal_generated.append(
                    (f"emit-c {lane}", str(generated_path))
                )
                all_binaries.append(
                    (f"emit-c {lane}", str(binary_path_value))
                )
            expected_commit = lane_state.get("expected_commit")
            for formal_build in (release, emit_c):
                if formal_build is None:
                    continue
                if formal_build.get("repository", {}).get("commit") != expected_commit:
                    raise HarnessError(
                        f"{workload_id} {lane} build is not bound to its exact commit"
                    )
                if formal_build.get("nomo", {}).get("sha256") != lane_state.get(
                    "nomo_sha256"
                ):
                    raise HarnessError(
                        f"{workload_id} {lane} build used a different compiler binary"
                    )
        for index, (left_label, left_path) in enumerate(formal_projects):
            for right_label, right_path in formal_projects[index + 1 :]:
                if artifact_paths_overlap(
                    left_path, right_path, recorded_host_os
                ):
                    raise HarnessError(
                        f"{workload_id} formal projects are reused or nested: "
                        f"{left_label}, {right_label}"
                    )
        formal_output_files = [
            *formal_generated,
            *formal_objects,
            *all_binaries,
        ]
        for index, (left_label, left_path) in enumerate(formal_output_files):
            for right_label, right_path in formal_output_files[index + 1 :]:
                if artifact_file_identity_equal(
                    left_path,
                    right_path,
                    recorded_host_os,
                    inspect_live_filesystem=live_filesystem,
                ):
                    raise HarnessError(
                        f"{workload_id} build artifact identity was reused: "
                        f"{left_label}, {right_label}"
                    )
        for reference_lane in REFERENCE_LANES:
            reference_path = binaries[reference_lane]["path"]
            for project_label, project_path in formal_projects:
                if artifact_paths_overlap(
                    reference_path, project_path, recorded_host_os
                ):
                    raise HarnessError(
                        f"{workload_id} reference {reference_lane} binary "
                        f"overlaps {project_label} project"
                    )


def validate_build_failure_evidence(
    result: Dict[str, Any],
    manifest: Dict[str, Any],
    *,
    live_filesystem: bool = True,
) -> None:
    failures = result.get("build_failures", [])
    if not isinstance(failures, list) or len(failures) != 1:
        raise HarnessError(
            "ineligible preflight build evidence requires exactly one failure"
        )
    failure = failures[0]
    workload_id = failure.get("workload_id")
    if workload_id not in WORKLOAD_IDS:
        raise HarnessError("preflight build failure workload changed")
    workload_index = WORKLOAD_IDS.index(str(workload_id))
    if set(result.get("builds", {})) != set(WORKLOAD_IDS[:workload_index]):
        raise HarnessError(
            "preflight build failure must retain the exact completed build prefix"
        )
    lane = failure.get("lane")
    phase = failure.get("phase")
    correctness_failure_phases = {
        *((reference_lane, "reference-build") for reference_lane in REFERENCE_LANES),
        ("nomo-baseline", "nomo-emit-c"),
        ("nomo-baseline", "nomo-generated-c-clang"),
    }
    formal_failure_phases = {
        *((formal_lane, formal_phase)
          for formal_lane in ("candidate", "main")
          for formal_phase in (
              "nomo-release-binary",
              "nomo-release-generated-c",
              "nomo-release-provenance",
              "nomo-emit-c",
              "nomo-generated-c-clang",
          )),
        *((reference_lane, "reference-build")
          for reference_lane in REFERENCE_LANES),
    }
    allowed_phases = (
        formal_failure_phases
        if result.get("mode") == "prepare"
        else correctness_failure_phases
    )
    if (lane, phase) not in allowed_phases:
        raise HarnessError("preflight build failure phase changed")
    workload = manifest["workloads"][workload_index]
    provenance = result.get("provenance", {})
    recorded_host_os = str(provenance.get("host", {}).get("os", ""))
    manifest_path_value = provenance.get("manifest_path")
    if not artifact_path_is_absolute(
        manifest_path_value, recorded_host_os
    ):
        raise HarnessError("preflight failure manifest path is not absolute")
    recorded_suite_root = artifact_path(
        manifest_path_value, recorded_host_os
    ).parent
    source = failure.get("source", {})
    source_path_value = source.get("path")
    source_path = artifact_path(source_path_value, recorded_host_os)
    if (
        not source_path.is_absolute()
        or re.fullmatch(r"[0-9a-f]{64}", str(source.get("sha256")))
        is None
    ):
        raise HarnessError("preflight build failure source identity changed")
    if live_filesystem:
        live_source_path = Path(str(source_path_value))
        if (
            not live_source_path.is_file()
            or v1.sha256_file(live_source_path) != source.get("sha256")
        ):
            raise HarnessError(
                "preflight build failure live source identity changed"
            )
    output_path_value = failure.get("output_path")
    output_path = artifact_path(output_path_value, recorded_host_os)
    command = failure.get("command", {})
    toolchains = provenance.get("toolchains", {})
    link_flags = math_flags_for_host(workload, recorded_host_os)
    expected_cwd = str(recorded_suite_root.parent.parent)
    if phase == "reference-build":
        if (
            source.get("sha256") != workload["sources"][lane]["sha256"]
            or source_path.name
            != {
                "c": "c.c",
                "cpp": "cpp.cpp",
                "semantic-c": "semantic-c.c",
                "go": "go.go",
            }[lane]
            or len(source_path.parents) < 4
        ):
            raise HarnessError("preflight reference source is not frozen")
        bundle_root = source_path.parents[3]
        expected_output = artifact_binary_path(
            bundle_root / "bin",
            f"{workload_id}-{lane}",
            recorded_host_os,
        )
        expected_argv = {
            "c": [
                toolchains.get("clang", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                str(source_path),
                "-o",
                str(output_path),
                *link_flags,
            ],
            "cpp": [
                toolchains.get("clangxx", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_CPP_FLAGS,
                str(source_path),
                "-o",
                str(output_path),
                *link_flags,
            ],
            "semantic-c": [
                toolchains.get("clang", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                str(source_path),
                "-o",
                str(output_path),
                *link_flags,
            ],
            "go": [
                toolchains.get("go", {}).get("path"),
                "build",
                "-o",
                str(output_path),
                str(source_path),
            ],
        }[lane]
    elif lane == "nomo-baseline" and phase == "nomo-emit-c":
        expected_source = (
            recorded_suite_root / workload["sources"]["nomo"]["path"]
        )
        if (
            source_path != expected_source
            or source.get("sha256") != workload["sources"]["nomo"]["sha256"]
            or len(output_path.parents) < 7
        ):
            raise HarnessError("failed Nomo emit-C source changed")
        bundle_root = output_path.parents[6]
        project = (
            bundle_root
            / "build"
            / workload_id
            / "references"
            / "nomo-baseline-project"
        )
        expected_output = project / "build" / "c" / "main.c"
        expected_argv = [
            toolchains.get("nomo", {}).get("path"),
            "build",
            str(project),
            "--emit-c",
        ]
    elif lane == "nomo-baseline":
        if len(source_path.parents) < 7:
            raise HarnessError("failed generated-C source is outside a bundle")
        bundle_root = source_path.parents[6]
        expected_source = (
            bundle_root
            / "build"
            / workload_id
            / "references"
            / "nomo-baseline-project"
            / "build"
            / "c"
            / "main.c"
        )
        expected_output = artifact_binary_path(
            bundle_root / "bin",
            f"{workload_id}-nomo-baseline",
            recorded_host_os,
        )
        if source_path != expected_source:
            raise HarnessError("failed generated-C source path changed")
        expected_argv = [
            toolchains.get("clang", {}).get("path"),
            *CLANG_DRIVER_CONFIG_FLAGS,
            *BASE_C_FLAGS,
            str(source_path),
            "-o",
            str(output_path),
            *link_flags,
        ]
    else:
        lane_state = result.get("release_lanes", {}).get(lane, {})
        checkout = artifact_path(
            lane_state.get("checkout"), recorded_host_os
        )
        if not checkout.is_absolute():
            raise HarnessError("formal build failure checkout is not absolute")
        expected_cwd = str(checkout)
        project = next(
            (
                parent
                for parent in output_path.parents
                if parent.name == "project"
            ),
            None,
        )
        if project is None or len(project.parents) < 5:
            raise HarnessError("formal build failure output is outside a project")
        build_mode = (
            "release"
            if str(phase).startswith("nomo-release-")
            else "emit-c"
        )
        bundle_root = project.parents[4]
        expected_project = (
            bundle_root
            / "build"
            / workload_id
            / build_mode
            / str(lane)
            / "project"
        )
        if project != expected_project:
            raise HarnessError("formal build failure project path changed")
        compiler_path = artifact_path(
            lane_state.get("nomo_path"), recorded_host_os
        )
        expected_compiler = artifact_binary_path(
            bundle_root / "compiler-build" / str(lane) / "release",
            "nomo",
            recorded_host_os,
        )
        if compiler_path != expected_compiler:
            raise HarnessError(
                "formal build failure used a compiler outside its build bundle"
            )
        canonical_nomo_source = (
            recorded_suite_root / workload["sources"]["nomo"]["path"]
        )
        generated_c = project / "build" / "c" / "main.c"
        project_name = (
            "benchmarksgame_" + str(workload_id).replace("-", "_")
        )
        formal_binary = artifact_binary_path(
            project / "build" / "bin",
            project_name,
            recorded_host_os,
        )
        if phase == "nomo-release-binary":
            expected_output = formal_binary
        elif phase == "nomo-release-generated-c":
            expected_output = generated_c
        elif phase == "nomo-release-provenance":
            expected_output = project / "build" / "release-provenance.json"
        elif phase == "nomo-emit-c":
            expected_output = generated_c
        else:
            expected_output = formal_binary
        if phase == "nomo-generated-c-clang":
            if source_path != generated_c:
                raise HarnessError("failed formal generated-C source changed")
            expected_argv = [
                toolchains.get("clang", {}).get("path"),
                *CLANG_DRIVER_CONFIG_FLAGS,
                *BASE_C_FLAGS,
                str(generated_c),
                "-o",
                str(output_path),
                *link_flags,
            ]
        else:
            if (
                source_path != canonical_nomo_source
                or source.get("sha256") != workload["sources"]["nomo"]["sha256"]
            ):
                raise HarnessError("failed formal Nomo source changed")
            expected_argv = [
                lane_state.get("nomo_path"),
                "build",
                str(project),
                "--release" if build_mode == "release" else "--emit-c",
            ]
    if (
        not output_path.is_absolute()
        or output_path != expected_output
    ):
        raise HarnessError("preflight build failure output path changed")
    if command.get("argv") != expected_argv or command.get(
        "command"
    ) != v1.command_text(expected_argv):
        raise HarnessError(
            "preflight build failure command changed: "
            f"expected {expected_argv!r}, found {command.get('argv')!r}"
        )
    if command.get("cwd") != expected_cwd:
        raise HarnessError("preflight build failure cwd changed")
    if (
        not isinstance(command.get("duration_ns"), int)
        or command["duration_ns"] < 0
        or not isinstance(command.get("error"), str)
        or not command["error"]
        or not isinstance(command.get("stdout"), str)
        or not isinstance(command.get("stderr"), str)
    ):
        raise HarnessError("preflight build failure record is incomplete")
    timed_out = command.get("timed_out")
    exit_code = command.get("exit_code")
    failure_kind = failure.get("failure_kind")
    output_state = failure.get("output_state")
    valid_output_kinds = {
        ("absent", "absent"),
        ("directory", "directory"),
        ("symlink", "symlink"),
        ("other-nonregular", "fifo"),
        ("other-nonregular", "socket"),
        ("other-nonregular", "character-device"),
        ("other-nonregular", "block-device"),
        ("other-nonregular", "unknown"),
    }
    if (
        not isinstance(output_state, dict)
        or set(output_state) != {"kind", "lstat_type", "symlink_target"}
        or (
            output_state.get("kind"),
            output_state.get("lstat_type"),
        )
        not in valid_output_kinds
        or (
            output_state.get("kind") == "symlink"
            and (
                not isinstance(output_state.get("symlink_target"), str)
                or not output_state["symlink_target"]
            )
        )
        or (
            output_state.get("kind") != "symlink"
            and output_state.get("symlink_target") is not None
        )
    ):
        raise HarnessError("preflight build output lstat evidence is invalid")
    if live_filesystem:
        live_output_state = build_output_state(
            Path(str(output_path_value))
        )
        if output_state != live_output_state:
            raise HarnessError(
                "preflight build output lstat evidence changed"
            )
    if failure_kind in {"missing-output", "invalid-output"}:
        expected_error = recorded_build_output_failure_error(
            str(output_path), output_state
        )
        if (
            timed_out is not False
            or exit_code != 0
            or command.get("error") != expected_error
            or (
                failure_kind == "missing-output"
                and output_state.get("kind") != "absent"
            )
            or (
                failure_kind == "invalid-output"
                and output_state.get("kind")
                not in {"directory", "symlink", "other-nonregular"}
            )
        ):
            raise HarnessError(
                f"{failure_kind} build evidence changed"
            )
    elif failure_kind == "command-failure":
        if timed_out is True:
            if exit_code is not None:
                raise HarnessError("timed-out preflight build has an exit code")
        elif timed_out is False:
            if exit_code == 0 or (
                exit_code is not None and not isinstance(exit_code, int)
            ):
                raise HarnessError("preflight build failure exit evidence changed")
        else:
            raise HarnessError("preflight build timeout evidence is missing")
    else:
        raise HarnessError("preflight build failure kind changed")
    if parse_utc_timestamp(command.get("started_at_utc")) > parse_utc_timestamp(
        command.get("finished_at_utc")
    ):
        raise HarnessError("preflight build failure UTC interval is invalid")
    go_cache_environment = {
        key: str(source_path.parent / directory)
        for key, directory in GO_BUILD_CACHE_DIRECTORIES.items()
    }
    validate_build_command_environment(
        command,
        f"{workload_id} failed {lane} build",
        go_cache_environment if lane == "go" else None,
        toolchains.get("build_environment"),
    )
    if live_filesystem and lane == "go" and any(
        Path(path).exists() for path in go_cache_environment.values()
    ):
        raise HarnessError("failed Go build cache was not removed")


def assert_recomputed_equal(actual: Any, expected: Any, path: str) -> None:
    if isinstance(expected, bool) or expected is None or isinstance(expected, str):
        if actual != expected:
            raise HarnessError(f"{path} does not match raw-sample recomputation")
        return
    if isinstance(expected, (int, float)):
        if not isinstance(actual, (int, float)) or isinstance(actual, bool):
            raise HarnessError(f"{path} does not match raw-sample recomputation")
        if not math.isclose(float(actual), float(expected), rel_tol=1e-12, abs_tol=1e-12):
            raise HarnessError(f"{path} does not match raw-sample recomputation")
        return
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            raise HarnessError(f"{path} does not match raw-sample recomputation")
        for index, (actual_item, expected_item) in enumerate(zip(actual, expected)):
            assert_recomputed_equal(actual_item, expected_item, f"{path}[{index}]")
        return
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or set(actual) != set(expected):
            raise HarnessError(f"{path} does not match raw-sample recomputation")
        for key in expected:
            assert_recomputed_equal(actual[key], expected[key], f"{path}.{key}")
        return
    raise HarnessError(f"{path} has an unsupported recomputation value")


def build_binary_record(
    builds: Dict[str, Any], workload_id: str, build_mode: str, lane: str
) -> Dict[str, Any]:
    build = builds.get(workload_id, {})
    if lane in ("candidate", "main"):
        return build.get("modes", {}).get(build_mode, {}).get(lane, {}).get(
            "binary", {}
        )
    return build.get("references", {}).get("binaries", {}).get(lane, {})


def parse_utc_timestamp(value: Any) -> dt.datetime:
    if not isinstance(value, str):
        raise HarnessError("UTC timestamp is missing")
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise HarnessError(f"invalid UTC timestamp: {value}") from error
    if parsed.tzinfo is None or parsed.utcoffset() != dt.timedelta(0):
        raise HarnessError(f"timestamp is not UTC: {value}")
    return parsed


def validate_collector_descriptor(
    descriptor: Dict[str, Any], host_os: str
) -> str:
    expected = collector_descriptor_for_host(host_os)
    if descriptor != expected:
        raise HarnessError(
            "collector descriptor does not match the host collector authority"
        )
    return expected["id"]


def validate_stdout_hash_evidence(
    sample: Dict[str, Any],
    *,
    expected_normalized_sha256: Optional[str] = None,
) -> bytes:
    encoded = sample.get("stdout_raw_base64")
    if not isinstance(encoded, str):
        raise HarnessError("sample lacks replayable raw stdout evidence")
    try:
        raw = base64.b64decode(encoded.encode("ascii"), validate=True)
    except (UnicodeEncodeError, ValueError, binascii.Error) as error:
        raise HarnessError("sample raw stdout evidence is not canonical base64") from error
    normalized = raw.replace(b"\r\n", b"\n")
    if (
        sample.get("stdout_raw_sha256") != v1.sha256_bytes(raw)
        or sample.get("stdout_normalized_sha256")
        != v1.sha256_bytes(normalized)
        or (
            expected_normalized_sha256 is not None
            and sample.get("stdout_normalized_sha256")
            != expected_normalized_sha256
        )
    ):
        raise HarnessError("sample stdout hashes do not replay from raw evidence")
    byte_counts = sample.get("stdout_bytes")
    if byte_counts is not None and byte_counts != {
        "raw": len(raw),
        "normalized": len(normalized),
    }:
        raise HarnessError("sample stdout byte counts do not match raw evidence")
    return normalized


def validate_sample_binding(
    sample: Dict[str, Any],
    binary: Dict[str, Any],
    input_value: str,
    fixture_sha256: str,
    collector_id: str,
    lane: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_host_os: Optional[str] = None,
) -> None:
    binary_path_value = binary.get("path")
    binary_sha = binary.get("sha256")
    if (
        not isinstance(binary_path_value, str)
        or not artifact_path_is_absolute(
            binary_path_value,
            recorded_host_os or platform.system(),
        )
        or re.fullmatch(r"[0-9a-f]{64}", str(binary_sha)) is None
    ):
        raise HarnessError(f"{lane} build binary identity is incomplete")
    expected_argv = [binary_path_value, input_value]
    if sample.get("command_argv") != expected_argv:
        raise HarnessError(f"{lane} sample switched executable or formal input")
    if sample.get("command") != v1.command_text(expected_argv):
        raise HarnessError(f"{lane} sample command rendering changed")
    if sample.get("executable_sha256") != binary_sha:
        raise HarnessError(f"{lane} sample executable SHA is not build-bound")
    if sample.get("stdout_normalized_sha256") != fixture_sha256:
        raise HarnessError(f"{lane} sample output does not match the fixed fixture")
    validate_stdout_hash_evidence(
        sample, expected_normalized_sha256=fixture_sha256
    )
    if sample.get("stdout_normalization") != STDOUT_NORMALIZATION:
        raise HarnessError(f"{lane} sample stdout normalization changed")
    if re.fullmatch(
        r"[0-9a-f]{64}", str(sample.get("stdout_raw_sha256"))
    ) is None:
        raise HarnessError(f"{lane} sample raw stdout SHA is missing")
    if sample.get("collector") != collector_id:
        raise HarnessError(f"{lane} sample collector changed")
    if (
        not isinstance(sample.get("user_cpu_ns"), int)
        or not isinstance(sample.get("system_cpu_ns"), int)
        or sample.get("cpu_total_ns")
        != sample["user_cpu_ns"] + sample["system_cpu_ns"]
    ):
        raise HarnessError(f"{lane} sample CPU total is not user plus system")
    expected_environment = (
        runtime_environments or {
            "default": _environment({})[1],
            "go": _environment({"GOMAXPROCS": "1"})[1],
        }
    )["go" if lane == "go" else "default"]
    if sample.get("environment") != expected_environment:
        raise HarnessError(f"{lane} sample environment changed")
    if parse_utc_timestamp(sample.get("started_at_utc")) > parse_utc_timestamp(
        sample.get("finished_at_utc")
    ):
        raise HarnessError(f"{lane} sample UTC interval is invalid")


def validate_correctness_evidence(
    items: Sequence[Dict[str, Any]],
    build_mode: str,
    lanes: Sequence[str],
    manifest: Dict[str, Any],
    builds: Dict[str, Any],
    collector_id: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_host_os: Optional[str] = None,
    recorded_suite_root: Optional[PurePath] = None,
) -> Optional[str]:
    workload_ids = tuple(item.get("id") for item in items)
    if (
        not workload_ids
        or len(workload_ids) > len(WORKLOAD_IDS)
        or workload_ids != WORKLOAD_IDS[: len(workload_ids)]
    ):
        raise HarnessError(
            "correctness evidence must be a nonempty frozen-order workload prefix"
        )
    workloads = {workload["id"]: workload for workload in manifest["workloads"]}
    host_os = recorded_host_os or platform.system()
    suite_root = recorded_suite_root or artifact_path(
        DEFAULT_MANIFEST.parent, host_os
    )
    failure_reason = None
    for item_index, item in enumerate(items):
        workload = workloads[item["id"]]
        fixture = suite_root / workload["fixtures"]["correctness"]["path"]
        if (
            item.get("build_mode") != build_mode
            or item.get("input") != workload["correctness_input"]
            or item.get("fixture_path") != str(fixture)
            or item.get("fixture_sha256")
            != workload["fixtures"]["correctness"]["sha256"]
            or tuple(item.get("lanes", [])) != tuple(lanes)
        ):
            raise HarnessError(f"{item['id']} correctness contract changed")
        status = item.get("status")
        attempted_lanes = tuple(item.get("attempted_lanes", []))
        implementations = item.get("implementations", {})
        if (
            not attempted_lanes
            or attempted_lanes != tuple(lanes[: len(attempted_lanes)])
            or set(implementations) != set(attempted_lanes)
        ):
            raise HarnessError(
                f"{item['id']} correctness lane prefix changed"
            )
        if status == "completed":
            if (
                attempted_lanes != tuple(lanes)
                or item.get("failure_reason") is not None
            ):
                raise HarnessError(
                    f"{item['id']} completed correctness evidence is incomplete"
                )
        elif status == "failed":
            if (
                item_index != len(items) - 1
                or failure_reason is not None
                or not isinstance(item.get("failure_reason"), str)
                or not item["failure_reason"]
            ):
                raise HarnessError(
                    "correctness failure must be the unique failed tail"
                )
            failure_reason = item["failure_reason"]
        else:
            raise HarnessError(f"{item['id']} correctness status is invalid")
        for lane_index, lane in enumerate(attempted_lanes):
            implementation = item["implementations"][lane]
            sample = implementation.get("sample", {})
            binary = build_binary_record(builds, item["id"], build_mode, lane)
            if build_mode == "baseline-emit-c":
                binary = build_binary_record(
                    builds, item["id"], "baseline-emit-c", lane
                )
            is_failed_tail = (
                status == "failed"
                and lane_index == len(attempted_lanes) - 1
            )
            if is_failed_tail:
                if (
                    implementation.get("passed") is not False
                    or implementation.get("stdout_normalized_sha256")
                    != sample.get("stdout_normalized_sha256")
                    or sample.get("failure_message") != failure_reason
                ):
                    raise HarnessError(
                        f"{item['id']} {lane} correctness failure evidence changed"
                    )
                validate_failed_sample_binding(
                    sample,
                    binary,
                    workload["correctness_input"],
                    workload["fixtures"]["correctness"]["sha256"],
                    collector_id,
                    lane,
                    runtime_environments,
                    host_os,
                )
            else:
                if implementation.get("passed") is not True:
                    raise HarnessError(
                        f"{item['id']} {lane} correctness did not pass"
                    )
                validate_sample_binding(
                    sample,
                    binary,
                    workload["correctness_input"],
                    workload["fixtures"]["correctness"]["sha256"],
                    collector_id,
                    lane,
                    runtime_environments,
                    host_os,
                )
                if (
                    implementation.get("stdout_normalized_sha256")
                    != workload["fixtures"]["correctness"]["sha256"]
                ):
                    raise HarnessError(
                        f"{item['id']} {lane} fixture hash changed"
                    )
        if status == "failed" and any(
            item["implementations"][lane].get("passed") is not True
            for lane in attempted_lanes[:-1]
        ):
            raise HarnessError(
                "correctness failure tail discarded an earlier lane failure"
            )
    if failure_reason is None and (
        workload_ids != WORKLOAD_IDS
        or any(item.get("status") != "completed" for item in items)
    ):
        raise HarnessError(
            "correctness evidence stopped without a structured failed tail"
        )
    return failure_reason


def validate_measured_workload(
    workload: Dict[str, Any],
    build_mode: str,
    batch_index: int,
    attempt_index: int,
    manifest: Dict[str, Any],
    builds: Dict[str, Any],
    collector_id: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_host_os: Optional[str] = None,
    recorded_suite_root: Optional[PurePath] = None,
) -> None:
    if workload.get("build_mode") != build_mode:
        raise HarnessError("measured workload build mode changed")
    manifest_workload = next(
        item for item in manifest["workloads"] if item["id"] == workload.get("id")
    )
    host_os = recorded_host_os or platform.system()
    suite_root = recorded_suite_root or artifact_path(
        DEFAULT_MANIFEST.parent, host_os
    )
    fixture = suite_root / manifest_workload["fixtures"]["performance"]["path"]
    if (
        workload.get("performance_input") != manifest_workload["performance_input"]
        or workload.get("fixture_path") != str(fixture)
        or workload.get("fixture_sha256")
        != manifest_workload["fixtures"]["performance"]["sha256"]
        or workload.get("collection_status") != "completed"
        or workload.get("failure_reason") is not None
    ):
        raise HarnessError("measured workload is not bound to the frozen input/fixture")
    schedule = williams_schedule(TIMED_LANES, 30)
    if workload.get("warmup_orders") != schedule[:2]:
        raise HarnessError("warmup order does not match the frozen Williams schedule")
    expected_blocks = [
        {"block_index": index, "order": order}
        for index, order in enumerate(schedule, start=1)
    ]
    if workload.get("block_schedule") != expected_blocks:
        raise HarnessError("block order does not match the frozen Williams schedule")
    for lane in TIMED_LANES:
        warmups = workload.get("warmups", {}).get(lane, [])
        samples = workload.get("samples", {}).get(lane, [])
        if len(warmups) != 2 or len(samples) != 30:
            raise HarnessError("every timed lane needs two warmups and 30 samples")
        for warmup_index, sample in enumerate(warmups, start=1):
            expected_position = schedule[warmup_index - 1].index(lane) + 1
            if (
                sample.get("phase") != "warmup"
                or sample.get("build_mode") != build_mode
                or sample.get("batch_index") != batch_index
                or sample.get("attempt_index") != attempt_index
                or sample.get("warmup_index") != warmup_index
                or sample.get("order_position") != expected_position
            ):
                raise HarnessError("warmup sample provenance does not match its schedule")
            if sample.get("command") != v1.command_text(
                sample.get("command_argv", [])
            ):
                raise HarnessError("warmup command provenance is not executable")
            validate_sample_binding(
                sample,
                build_binary_record(
                    builds, workload["id"], build_mode, lane
                ),
                manifest_workload["performance_input"],
                manifest_workload["fixtures"]["performance"]["sha256"],
                collector_id,
                lane,
                runtime_environments,
                host_os,
            )
        for block_index, sample in enumerate(samples, start=1):
            expected_position = schedule[block_index - 1].index(lane) + 1
            if (
                sample.get("phase") != "sample"
                or sample.get("build_mode") != build_mode
                or sample.get("batch_index") != batch_index
                or sample.get("attempt_index") != attempt_index
                or sample.get("block_index") != block_index
                or sample.get("order_position") != expected_position
            ):
                raise HarnessError("timed sample provenance does not match its block")
            if sample.get("command") != v1.command_text(
                sample.get("command_argv", [])
            ):
                raise HarnessError("timed command provenance is not executable")
            validate_sample_binding(
                sample,
                build_binary_record(
                    builds, workload["id"], build_mode, lane
                ),
                manifest_workload["performance_input"],
                manifest_workload["fixtures"]["performance"]["sha256"],
                collector_id,
                lane,
                runtime_environments,
                host_os,
            )


def validate_partial_workload(
    workload: Dict[str, Any],
    build_mode: str,
    batch_index: int,
    attempt_index: int,
    manifest: Dict[str, Any],
    builds: Dict[str, Any],
    collector_id: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_host_os: Optional[str] = None,
    recorded_suite_root: Optional[PurePath] = None,
) -> None:
    if (
        workload.get("build_mode") != build_mode
        or workload.get("collection_status") != "failed"
        or not workload.get("failure_reason")
    ):
        raise HarnessError("partial workload must retain a failure reason")
    manifest_workload = next(
        item for item in manifest["workloads"] if item["id"] == workload.get("id")
    )
    host_os = recorded_host_os or platform.system()
    suite_root = recorded_suite_root or artifact_path(
        DEFAULT_MANIFEST.parent, host_os
    )
    fixture = suite_root / manifest_workload["fixtures"]["performance"]["path"]
    schedule = williams_schedule(TIMED_LANES, 30)
    if (
        workload.get("performance_input") != manifest_workload["performance_input"]
        or workload.get("fixture_path") != str(fixture)
        or workload.get("fixture_sha256")
        != manifest_workload["fixtures"]["performance"]["sha256"]
        or workload.get("warmup_orders") != schedule[:2]
    ):
        raise HarnessError("partial workload is not bound to the frozen contract")
    observed_events: list[tuple[int, str, Dict[str, Any]]] = []
    failed_samples = []
    for lane in TIMED_LANES:
        for sample in [
            *workload.get("warmups", {}).get(lane, []),
            *workload.get("samples", {}).get(lane, []),
        ]:
            binary = build_binary_record(
                builds, workload["id"], build_mode, lane
            )
            phase = sample.get("phase")
            order_position = sample.get("order_position")
            if phase == "warmup":
                event_index = sample.get("warmup_index")
                if (
                    not isinstance(event_index, int)
                    or not 1 <= event_index <= 2
                    or not isinstance(order_position, int)
                    or not 1 <= order_position <= 5
                    or schedule[event_index - 1][order_position - 1] != lane
                ):
                    raise HarnessError("partial warmup schedule provenance changed")
                ordinal = (event_index - 1) * 5 + order_position - 1
            elif phase == "sample":
                event_index = sample.get("block_index")
                if (
                    not isinstance(event_index, int)
                    or not 1 <= event_index <= 30
                    or not isinstance(order_position, int)
                    or not 1 <= order_position <= 5
                    or schedule[event_index - 1][order_position - 1] != lane
                ):
                    raise HarnessError("partial timed schedule provenance changed")
                ordinal = 10 + (event_index - 1) * 5 + order_position - 1
            else:
                raise HarnessError("partial sample phase is invalid")
            if (
                sample.get("build_mode") != build_mode
                or sample.get("batch_index") != batch_index
                or sample.get("attempt_index") != attempt_index
            ):
                raise HarnessError("partial sample batch provenance changed")
            observed_events.append((ordinal, lane, sample))
            if sample.get("status") == "failed":
                failed_samples.append(sample)
                validate_failed_sample_binding(
                    sample,
                    binary,
                    manifest_workload["performance_input"],
                    manifest_workload["fixtures"]["performance"]["sha256"],
                    collector_id,
                    lane,
                    runtime_environments,
                    host_os,
                )
                if sample.get("failure_message") != workload.get(
                    "failure_reason"
                ):
                    raise HarnessError(
                        "partial failure reason does not match failed sample"
                    )
            else:
                validate_sample_binding(
                    sample,
                    binary,
                    manifest_workload["performance_input"],
                    manifest_workload["fixtures"]["performance"]["sha256"],
                    collector_id,
                    lane,
                    runtime_environments,
                    host_os,
                )
    observed_events.sort(key=lambda item: item[0])
    ordinals = [item[0] for item in observed_events]
    if (
        len(failed_samples) != 1
        or not ordinals
        or ordinals != list(range(ordinals[-1] + 1))
        or observed_events[-1][2].get("status") != "failed"
    ):
        raise HarnessError(
            "partial workload must retain one failed tail after a complete event prefix"
        )
    failed = failed_samples[0]
    expected_block_schedule = []
    if failed.get("phase") == "sample":
        expected_block_schedule = [
            {"block_index": index, "order": order}
            for index, order in enumerate(
                schedule[: failed["block_index"]], start=1
            )
        ]
    if workload.get("block_schedule") != expected_block_schedule:
        raise HarnessError("partial workload block schedule changed")


def validate_failed_sample_binding(
    sample: Dict[str, Any],
    binary: Dict[str, Any],
    input_value: str,
    fixture_sha256: str,
    collector_id: str,
    lane: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_host_os: Optional[str] = None,
) -> None:
    expected_argv = [binary.get("path"), input_value]
    expected_environment = (
        runtime_environments or {
            "default": _environment({})[1],
            "go": _environment({"GOMAXPROCS": "1"})[1],
        }
    )["go" if lane == "go" else "default"]
    if (
        sample.get("status") != "failed"
        or sample.get("command_argv") != expected_argv
        or sample.get("command") != v1.command_text(expected_argv)
        or sample.get("executable_sha256") != binary.get("sha256")
        or sample.get("collector") != collector_id
        or sample.get("environment") != expected_environment
        or sample.get("stdout_normalization") != STDOUT_NORMALIZATION
        or sample.get("failure_kind")
        not in {
            "timeout",
            "output-mismatch",
            "process-failure",
            "collector-failure",
        }
        or not isinstance(sample.get("failure_source"), str)
        or not sample.get("failure_source")
        or re.fullmatch(
            r"[0-9a-f]{64}", str(sample.get("stdout_raw_sha256"))
        )
        is None
        or re.fullmatch(
            r"[0-9a-f]{64}", str(sample.get("stdout_normalized_sha256"))
        )
        is None
        or re.fullmatch(
            r"[0-9a-f]{64}",
            str(sample.get("stderr", {}).get("sha256")),
        )
        is None
    ):
        raise HarnessError(f"{lane} failed sample evidence is not build-bound")
    if (sample["failure_kind"] == "timeout") != bool(sample.get("timed_out")):
        raise HarnessError(f"{lane} failed sample timeout state is inconsistent")
    if parse_utc_timestamp(sample.get("started_at_utc")) > parse_utc_timestamp(
        sample.get("finished_at_utc")
    ):
        raise HarnessError(f"{lane} failed sample UTC interval is invalid")
    if (
        sample.get("stdout") != "captured-failed"
        or not isinstance(sample.get("wall_ns"), int)
        or sample["wall_ns"] <= 0
        or not isinstance(sample.get("stdout_bytes", {}).get("raw"), int)
        or not isinstance(
            sample.get("stdout_bytes", {}).get("normalized"), int
        )
        or not isinstance(sample.get("stderr", {}).get("length_bytes"), int)
    ):
        raise HarnessError(f"{lane} failed sample raw evidence is incomplete")
    validate_stdout_hash_evidence(sample)
    cpu_values = (sample.get("user_cpu_ns"), sample.get("system_cpu_ns"))
    expected_cpu_total = (
        cpu_values[0] + cpu_values[1]
        if all(isinstance(value, int) for value in cpu_values)
        else None
    )
    if sample.get("cpu_total_ns") != expected_cpu_total:
        raise HarnessError(f"{lane} failed sample CPU evidence is inconsistent")
    failure_kind = sample["failure_kind"]
    exit_code = sample.get("exit_code")
    stderr_length = sample["stderr"]["length_bytes"]
    stderr_sha256 = sample["stderr"]["sha256"]
    if failure_kind == "output-mismatch":
        if (
            exit_code != 0
            or stderr_length != 0
            or stderr_sha256 != v1.sha256_bytes(b"")
            or sample["stdout_normalized_sha256"] == fixture_sha256
        ):
            raise HarnessError(
                f"{lane} output-mismatch kind is not supported by raw evidence"
            )
    elif failure_kind == "process-failure":
        if not (
            isinstance(exit_code, int)
            and exit_code != 0
            or stderr_length > 0
        ):
            raise HarnessError(
                f"{lane} process-failure kind is not supported by raw evidence"
            )
    elif failure_kind == "timeout" and exit_code == 0:
        raise HarnessError(
            f"{lane} timeout kind cannot report a successful process exit"
        )


def dynamic_command_matches(
    observation: Dict[str, Any], executable: str, arguments: Sequence[str]
) -> bool:
    command = observation.get("command_argv")
    if not isinstance(command, list) or len(command) != len(arguments) + 1:
        return False
    actual_name = (
        str(command[0])
        .replace("\\", "/")
        .rsplit("/", 1)[-1]
        .lower()
        .removesuffix(".exe")
    )
    return (
        actual_name == executable.lower().removesuffix(".exe")
        and command[1:] == list(arguments)
    )


def normalized_executable_path(value: str) -> str:
    return str(value).replace("\\", "/").rstrip("/").casefold()


def expected_dynamic_system_path(
    host_os: str,
    executable: str,
    *,
    windows_system_root: Optional[str] = None,
    live_authority: bool = True,
) -> Optional[str]:
    if host_os == "Darwin":
        return {
            "pmset": DARWIN_PMSET,
            "osascript": DARWIN_OSASCRIPT,
            "sysctl": DARWIN_SYSCTL,
        }.get(executable)
    if host_os == "Windows" and executable == "powercfg":
        if windows_system_root is not None:
            root = artifact_path(windows_system_root, "Windows")
            if not root.is_absolute():
                return None
            return str(root / "System32" / "powercfg.exe")
        if live_authority:
            return str(windows_system_directory() / "powercfg.exe")
    return None


def dynamic_command_matches_system_path(
    observation: Dict[str, Any],
    host_os: str,
    executable: str,
    arguments: Sequence[str],
    *,
    windows_system_root: Optional[str] = None,
    live_authority: bool = True,
) -> bool:
    expected = expected_dynamic_system_path(
        host_os,
        executable,
        windows_system_root=windows_system_root,
        live_authority=live_authority,
    )
    command = observation.get("command_argv")
    identity = observation.get("command_identity")
    if (
        expected is None
        or not dynamic_command_matches(observation, executable, arguments)
        or not isinstance(command, list)
        or not isinstance(identity, dict)
    ):
        return False
    expected_normalized = normalized_executable_path(expected)
    return (
        normalized_executable_path(str(command[0])) == expected_normalized
        and normalized_executable_path(str(identity.get("path")))
        == expected_normalized
        and normalized_executable_path(str(identity.get("realpath")))
        == expected_normalized
    )


def validate_dynamic_command_evidence(
    observation: Dict[str, Any],
    *,
    host_os: Optional[str] = None,
    windows_system_root: Optional[str] = None,
    live_filesystem: bool = True,
) -> None:
    producer_os = host_os or platform.system()
    environment = observation.get("environment")
    if (
        not isinstance(environment, dict)
        or set(environment) != {"LC_ALL", "LANG", "PATH"}
        or environment.get("LC_ALL") != "C"
        or environment.get("LANG") != "C"
        or not isinstance(environment.get("PATH"), str)
        or not environment["PATH"]
    ):
        raise HarnessError(
            "dynamic environment command did not use the controlled locale and PATH"
        )
    path_separator = ";" if producer_os == "Windows" else ":"
    environment_paths = environment["PATH"].split(path_separator)
    if (
        not environment_paths
        or any(
            not artifact_path(path, producer_os).is_absolute()
            for path in environment_paths
        )
    ):
        raise HarnessError(
            "dynamic environment command PATH is not producer-absolute"
        )
    identity = observation.get("command_identity")
    if identity is None:
        if observation.get("status") != "failed":
            raise HarnessError(
                "qualified dynamic command lacks executable identity"
            )
        return
    if not isinstance(identity, dict) or set(identity) != {
        "path",
        "realpath",
        "sha256",
        "version_output",
    }:
        raise HarnessError(
            "dynamic environment command identity is incomplete"
        )
    path_value = str(identity.get("path"))
    realpath_value = str(identity.get("realpath"))
    path = artifact_path(path_value, producer_os)
    realpath = artifact_path(realpath_value, producer_os)
    command = observation.get("command_argv")
    if (
        not path.is_absolute()
        or not realpath.is_absolute()
        or normalized_executable_path(path_value)
        != normalized_executable_path(realpath_value)
        or re.fullmatch(r"[0-9a-f]{64}", str(identity.get("sha256")))
        is None
        or not isinstance(command, list)
        or not command
        or normalized_executable_path(str(command[0]))
        != normalized_executable_path(path_value)
        or identity.get("version_output") is not None
    ):
        raise HarnessError(
            "dynamic environment command identity is incomplete or inconsistent"
        )
    path_parent = normalized_executable_path(str(path.parent))
    if path_parent not in {
        normalized_executable_path(value) for value in environment_paths
    }:
        raise HarnessError(
            "dynamic environment command directory is outside controlled PATH"
        )
    if not live_filesystem:
        return
    if producer_os != platform.system():
        raise HarnessError(
            "live dynamic command validation cannot cross producer hosts"
        )
    expected_environment = dynamic_command_environment()
    live_path = Path(path_value)
    live_realpath = Path(realpath_value)
    controlled_resolution = (
        _windows_executable_from_path(live_path.name, stable_build_path())
        if os.name == "nt"
        else shutil.which(live_path.name, path=stable_build_path())
    )
    if (
        environment != expected_environment
        or not live_path.is_file()
        or live_path.resolve() != live_realpath
        or not live_realpath.is_file()
        or v1.sha256_file(live_realpath) != identity.get("sha256")
        or controlled_resolution is None
        or Path(controlled_resolution).resolve() != live_realpath
    ):
        raise HarnessError(
            "dynamic environment command identity is not live and controlled"
        )


def parse_dynamic_observation_from_raw(
    observation_id: str,
    observation: Dict[str, Any],
    policy: Dict[str, Any],
) -> Any:
    raw_text = observation.get("raw", {}).get("text")
    if not isinstance(raw_text, str):
        return None
    source = observation.get("source")
    if source == "command":
        if dynamic_command_matches(
            observation, "pmset", ["-g", "batt"]
        ) and observation_id == "power_mode":
            return {"ac_power": "Now drawing from 'AC Power'" in raw_text}
        if dynamic_command_matches(
            observation, "pmset", ["-g"]
        ) and observation_id == "low_power_mode":
            match = re.search(
                r"^\s*lowpowermode\s+(\d+)\s*$", raw_text, re.MULTILINE
            )
            return {
                "enabled": None if match is None else int(match.group(1)) != 0
            }
        if dynamic_command_matches(
            observation, "pmset", ["-g", "therm"]
        ) and observation_id == "frequency_governor":
            return parse_darwin_frequency_observation(raw_text)
        if (
            dynamic_command_matches(
                observation,
                "osascript",
                ["-l", "JavaScript", "-e", DARWIN_THERMAL_STATE_SCRIPT],
            )
            and observation_id == "thermal_state"
        ):
            return parse_darwin_process_thermal_state(raw_text)
        if (
            dynamic_command_matches(
                observation, "sysctl", ["-n", "vm.swapusage"]
            )
            and observation_id == "swap"
        ):
            match = re.search(r"used\s*=\s*([0-9.]+)([KMGT])", raw_text)
            return {
                "used_bytes": (
                    _parse_byte_quantity(match.group(1), match.group(2))
                    if match
                    else None
                )
            }
        return None
    if source == "procfs" and observation_id == "swap":
        match = re.search(
            r"SwapTotal:\s*(\d+)\s*kB.*SwapFree:\s*(\d+)\s*kB",
            raw_text,
            re.DOTALL,
        )
        return {
            "used_bytes": (
                (int(match.group(1)) - int(match.group(2))) * 1024
                if match
                else None
            )
        }
    try:
        raw_value = json.loads(raw_text)
    except json.JSONDecodeError:
        return None
    if observation_id == "concurrent_load":
        if not isinstance(raw_value, dict):
            return None
        loads = raw_value.get("load_average")
        logical_cores = raw_value.get("logical_cores")
        if (
            not isinstance(loads, list)
            or not loads
            or not isinstance(logical_cores, int)
            or logical_cores <= 0
        ):
            return None
        normalized = loads[0] / logical_cores
        return {
            "load_average": loads,
            "logical_cores": logical_cores,
            "one_minute_per_logical_core": normalized,
            "failure_threshold": policy["max_load_per_logical_core"],
        }
    if source == "sysfs":
        if observation_id == "frequency_governor" and isinstance(
            raw_value, list
        ):
            return {"governors": raw_value}
        if observation_id == "low_power_mode" and isinstance(raw_value, list):
            return {
                "enabled": (
                    None
                    if not raw_value
                    else any(value != "performance" for value in raw_value)
                )
            }
        if observation_id == "power_mode" and isinstance(raw_value, dict):
            return {
                "ac_power": (
                    None
                    if not raw_value
                    else any(value == "1" for value in raw_value.values())
                )
            }
        if observation_id == "thermal_state" and isinstance(raw_value, list):
            return {
                "temperatures_celsius": raw_value,
                "maximum_celsius": max(raw_value) if raw_value else None,
            }
    if observation_id == "affinity":
        if source == "os.sched_getaffinity" and isinstance(raw_value, list):
            return {"cpus": raw_value}
        if source == "system-api" and raw_value == {"supported": False}:
            return {"supported": False, "enforced": False}
    return None


def dynamic_observation_is_qualified(
    observation_id: str,
    observation: Dict[str, Any],
    policy: Dict[str, Any],
    host_os: Optional[str] = None,
    host_architecture: Optional[str] = None,
) -> bool:
    parsed = observation.get("parsed")
    if not isinstance(parsed, dict):
        return False
    if observation_id == "power_mode":
        return (
            parsed.get("ac_power") is True
            if policy["require_ac_power"]
            else isinstance(parsed.get("ac_power"), bool)
        )
    if observation_id == "low_power_mode":
        return isinstance(parsed.get("enabled"), bool) and (
            policy["allow_low_power_mode"] or parsed["enabled"] is False
        )
    if observation_id == "frequency_governor":
        governors = parsed.get("governors")
        if isinstance(governors, list):
            return bool(governors) and all(
                isinstance(value, str)
                and value in policy["allowed_linux_governors"]
                for value in governors
            )
        if parsed.get("applicability") == "not-applicable":
            auxiliary = parsed.get("auxiliary_pmset")
            return (
                host_os == "Darwin"
                and host_architecture == "arm64"
                and parsed.get("platform") == "Darwin-Apple-Silicon"
                and parsed.get("governor_exposed") is False
                and isinstance(auxiliary, dict)
                and auxiliary.get("recognized_complete") is True
                and auxiliary.get("explicit_degradation") is False
                and auxiliary.get("cpu_speed_limit_percent")
                in {None, 100}
                and auxiliary.get("scheduler_limit_percent")
                in {None, 100}
            )
        return (
            parsed.get("thermal_warning_normal") is True
            and parsed.get("cpu_speed_limit_percent") == 100
        )
    if observation_id == "thermal_state":
        normal = parsed.get("normal")
        maximum = parsed.get("maximum_celsius")
        normal_valid = normal is True if normal is not None else maximum is not None
        maximum_valid = (
            maximum is None
            or (
                isinstance(maximum, (int, float))
                and math.isfinite(float(maximum))
                and maximum <= policy["max_thermal_celsius"]
            )
        )
        return normal_valid and maximum_valid
    if observation_id == "concurrent_load":
        normalized = parsed.get("one_minute_per_logical_core")
        return (
            isinstance(normalized, (int, float))
            and math.isfinite(float(normalized))
            and 0 <= normalized < policy["max_load_per_logical_core"]
        )
    if observation_id == "swap":
        return isinstance(parsed.get("used_bytes"), int) and parsed["used_bytes"] >= 0
    if observation_id == "affinity":
        cpus = parsed.get("cpus")
        if isinstance(cpus, list):
            return bool(cpus) and all(isinstance(cpu, int) and cpu >= 0 for cpu in cpus)
        return (
            parsed.get("supported") is False
            and parsed.get("enforced") is False
        )
    return False


def dynamic_source_profile_is_allowed(
    host_os: str,
    observation_id: str,
    observation: Dict[str, Any],
    *,
    windows_system_root: Optional[str] = None,
    live_authority: bool = True,
) -> bool:
    source = observation.get("source")
    command = observation.get("command_argv")
    common = (
        observation_id == "concurrent_load" and source == "os.getloadavg"
    )
    if common:
        return True
    if host_os == "Darwin":
        expected_commands = {
            "power_mode": ("pmset", ["-g", "batt"]),
            "low_power_mode": ("pmset", ["-g"]),
            "frequency_governor": ("pmset", ["-g", "therm"]),
            "thermal_state": (
                "osascript",
                ["-l", "JavaScript", "-e", DARWIN_THERMAL_STATE_SCRIPT],
            ),
            "swap": ("sysctl", ["-n", "vm.swapusage"]),
        }
        return (
            dynamic_command_matches_system_path(
                observation,
                host_os,
                *expected_commands[observation_id],
                live_authority=live_authority,
            )
            if observation_id in expected_commands
            else observation_id == "affinity" and source == "system-api"
        )
    if host_os == "Linux":
        return (
            observation_id
            in {
                "power_mode",
                "low_power_mode",
                "frequency_governor",
                "thermal_state",
            }
            and source == "sysfs"
            or observation_id == "swap"
            and source == "procfs"
            or observation_id == "affinity"
            and source == "os.sched_getaffinity"
        )
    if host_os == "Windows":
        return (
            observation_id in {"power_mode", "low_power_mode"}
            and dynamic_command_matches_system_path(
                observation,
                host_os,
                "powercfg",
                ["/getactivescheme"],
                windows_system_root=windows_system_root,
                live_authority=live_authority,
            )
            or observation_id in {"frequency_governor", "thermal_state", "affinity"}
            and source == "system-api"
            or observation_id == "swap"
            and source == "GlobalMemoryStatusEx"
        )
    return False


def validate_dynamic_snapshot(
    snapshot: Dict[str, Any],
    static_authorization: Dict[str, Any],
    host_os: str,
    host_architecture: str,
    *,
    windows_system_root: Optional[str] = None,
    live_authority: bool = True,
) -> None:
    required = {
        "schema",
        "captured_at_utc",
        "monotonic_ns",
        "authority_host_sha256",
        "observed_host_sha256",
        "observations",
        "policy",
        "eligible",
        "reason",
        "snapshot_sha256",
    }
    if set(snapshot) != required or snapshot.get("schema") != 1:
        raise HarnessError("dynamic environment snapshot structure changed")
    body = {key: value for key, value in snapshot.items() if key != "snapshot_sha256"}
    if snapshot["snapshot_sha256"] != canonical_json_sha256(body):
        raise HarnessError("dynamic environment snapshot digest is invalid")
    expected_host = static_authorization["expected_bindings"][
        "authority_host_sha256"
    ]
    if (
        snapshot["authority_host_sha256"] != expected_host
        or snapshot["observed_host_sha256"] != expected_host
    ):
        raise HarnessError("dynamic environment snapshot is cross-host")
    if set(snapshot["observations"]) != {
        "power_mode",
        "low_power_mode",
        "frequency_governor",
        "thermal_state",
        "concurrent_load",
        "swap",
        "affinity",
    }:
        raise HarnessError("dynamic environment observations are incomplete")
    if snapshot["policy"] != DYNAMIC_ENVIRONMENT_POLICY:
        raise HarnessError("dynamic environment snapshot policy changed")
    recomputed_statuses = {}
    command_environment: Optional[Dict[str, str]] = None
    for observation_id, observation in snapshot["observations"].items():
        if not dynamic_source_profile_is_allowed(
            host_os,
            observation_id,
            observation,
            windows_system_root=windows_system_root,
            live_authority=live_authority,
        ):
            raise HarnessError(
                f"dynamic environment {observation_id} source is not allowed "
                f"for {host_os}"
            )
        raw = observation.get("raw", {})
        raw_text = raw.get("text")
        raw_encoded = (
            raw_text.encode("utf-8") if isinstance(raw_text, str) else None
        )
        if (
            observation.get("status")
            not in {"qualified", "failed"}
            or not observation.get("source")
            or not isinstance(observation.get("reason"), str)
            or re.fullmatch(r"[0-9a-f]{64}", str(raw.get("sha256"))) is None
            or raw_encoded is None
            or raw.get("length_bytes") != len(raw_encoded)
            or raw.get("sha256") != v1.sha256_bytes(raw_encoded)
            or "parsed" not in observation
        ):
            raise HarnessError("dynamic environment raw/parsed evidence is incomplete")
        if (
            observation.get("source") == "command"
            and observation.get("status") == "qualified"
        ):
            if observation.get("exit_code") != 0:
                raise HarnessError(
                    f"dynamic environment {observation_id} command did not succeed"
                )
        if observation.get("source") == "command":
            validate_dynamic_command_evidence(
                observation,
                host_os=host_os,
                windows_system_root=windows_system_root,
                live_filesystem=live_authority,
            )
            if command_environment is None:
                command_environment = observation["environment"]
            elif observation["environment"] != command_environment:
                raise HarnessError(
                    "dynamic environment commands used different environments"
                )
        reparsed = parse_dynamic_observation_from_raw(
            observation_id, observation, snapshot["policy"]
        )
        if observation.get("parsed") != reparsed:
            raise HarnessError(
                f"dynamic environment {observation_id} parsed evidence "
                "does not match raw content"
            )
        recomputed_qualified = dynamic_observation_is_qualified(
            observation_id,
            {**observation, "parsed": reparsed},
            snapshot["policy"],
            host_os,
            host_architecture,
        )
        recomputed_statuses[observation_id] = recomputed_qualified
        expected_status = "qualified" if recomputed_qualified else "failed"
        if observation.get("status") != expected_status:
            raise HarnessError(
                f"dynamic environment {observation_id} status was not recomputed"
            )
    recomputed_eligible = (
        snapshot["authority_host_sha256"] == expected_host
        and snapshot["observed_host_sha256"] == expected_host
        and all(recomputed_statuses.values())
    )
    if snapshot.get("eligible") is not recomputed_eligible:
        raise HarnessError("dynamic environment eligibility was not recomputed")


def validate_protocol(
    protocol: Dict[str, Any],
    build_mode: str,
    manifest: Dict[str, Any],
    builds: Dict[str, Any],
    collector_id: str,
    static_authorization: Dict[str, Any],
    host_os: str,
    host_architecture: str,
    runtime_environments: Optional[Dict[str, Any]] = None,
    recorded_suite_root: Optional[PurePath] = None,
    *,
    live_authority: bool = True,
) -> set[str]:
    if protocol.get("build_mode") != build_mode:
        raise HarnessError(f"{build_mode} protocol identity changed")
    correctness_failure = None
    if protocol.get("correctness"):
        correctness_failure = validate_correctness_evidence(
            protocol["correctness"],
            build_mode,
            CORRECTNESS_FORMAL_LANES,
            manifest,
            builds,
            collector_id,
            runtime_environments,
            host_os,
            recorded_suite_root,
        )
    batches = protocol.get("batches", [])
    if len(batches) > 3:
        raise HarnessError(f"{build_mode} exceeded one automatic rerun")
    completed_count = 0
    invalidated_count = 0
    snapshot_ids: set[str] = set()
    for attempt_index, batch in enumerate(batches, start=1):
        expected_batch_index = completed_count + 1
        if (
            batch.get("build_mode") != build_mode
            or batch.get("attempt_index") != attempt_index
            or batch.get("batch_index") != expected_batch_index
        ):
            raise HarnessError(f"{build_mode} batch/attempt sequence is invalid")
        if batch.get("static_authorization_sha256") != static_authorization.get(
            "qualification_sha256"
        ):
            raise HarnessError(f"{build_mode} batch static authorization changed")
        environment_before = batch.get("dynamic_environment_before", {})
        environment_after = batch.get("dynamic_environment_after", {})
        validate_dynamic_snapshot(
            environment_before,
            static_authorization,
            host_os,
            host_architecture,
            windows_system_root=(
                runtime_environments.get("default", {}).get("SystemRoot")
                if isinstance(runtime_environments, dict)
                else None
            ),
            live_authority=live_authority,
        )
        validate_dynamic_snapshot(
            environment_after,
            static_authorization,
            host_os,
            host_architecture,
            windows_system_root=(
                runtime_environments.get("default", {}).get("SystemRoot")
                if isinstance(runtime_environments, dict)
                else None
            ),
            live_authority=live_authority,
        )
        for snapshot in (environment_before, environment_after):
            if snapshot["snapshot_sha256"] in snapshot_ids:
                raise HarnessError("dynamic environment snapshot was reused")
            snapshot_ids.add(snapshot["snapshot_sha256"])
        pair_issues = dynamic_environment_pair_issues(
            environment_before,
            environment_after,
            static_authorization["dynamic_policy"],
        )
        environment_valid = not pair_issues
        workloads = batch.get("workloads", [])
        workload_ids = tuple(item.get("id") for item in workloads)
        if workload_ids != WORKLOAD_IDS[: len(workloads)]:
            raise HarnessError(
                "partial batch workloads must be a frozen-order prefix"
            )
        has_complete_raw_samples = (
            len(workloads) == 3
            and all(
                item.get("collection_status") == "completed"
                for item in workloads
            )
        )
        if has_complete_raw_samples:
            for workload in workloads:
                validate_measured_workload(
                    workload,
                    build_mode,
                    expected_batch_index,
                    attempt_index,
                    manifest,
                    builds,
                    collector_id,
                    runtime_environments,
                    host_os,
                    recorded_suite_root,
                )
            recomputed_stability = batch_stability(
                workloads, manifest["methodology"]["batch_invalidation"]
            )
            if batch.get("stability") is None:
                raise HarnessError("complete raw samples require stability evidence")
            assert_recomputed_equal(
                batch["stability"],
                recomputed_stability,
                f"{build_mode}.batches[{attempt_index - 1}].stability",
            )
        else:
            failed_positions = [
                index
                for index, workload in enumerate(workloads)
                if workload.get("collection_status") == "failed"
            ]
            if workloads and (
                failed_positions != [len(workloads) - 1]
                or any(
                    workload.get("collection_status") != "completed"
                    for workload in workloads[:-1]
                )
            ):
                raise HarnessError(
                    "partial batch requires a completed prefix and one failed tail"
                )
            if batch.get("stability") is not None:
                raise HarnessError(
                    "partial raw samples cannot contain stability summaries"
                )
            for workload in workloads[:-1]:
                validate_measured_workload(
                    workload,
                    build_mode,
                    expected_batch_index,
                    attempt_index,
                    manifest,
                    builds,
                    collector_id,
                    runtime_environments,
                    host_os,
                    recorded_suite_root,
                )
            if workloads:
                validate_partial_workload(
                    workloads[-1],
                    build_mode,
                    expected_batch_index,
                    attempt_index,
                    manifest,
                    builds,
                    collector_id,
                    runtime_environments,
                    host_os,
                    recorded_suite_root,
                )
        snapshot_before_utc = parse_utc_timestamp(
            environment_before["captured_at_utc"]
        )
        batch_started_utc = parse_utc_timestamp(batch.get("started_at_utc"))
        snapshot_after_utc = parse_utc_timestamp(
            environment_after["captured_at_utc"]
        )
        batch_finished_utc = parse_utc_timestamp(batch.get("finished_at_utc"))
        sample_intervals = [
            (
                parse_utc_timestamp(sample.get("started_at_utc")),
                parse_utc_timestamp(sample.get("finished_at_utc")),
            )
            for workload in workloads
            for phase in ("warmups", "samples")
            for lane_samples in workload.get(phase, {}).values()
            for sample in lane_samples
        ]
        bracketed = (
            batch_started_utc
            <= snapshot_before_utc
            <= snapshot_after_utc
            <= batch_finished_utc
        )
        if sample_intervals:
            bracketed = (
                batch_started_utc <= snapshot_before_utc
                and all(
                    snapshot_before_utc
                    <= sample_started
                    <= sample_finished
                    <= snapshot_after_utc
                    for sample_started, sample_finished in sample_intervals
                )
                and snapshot_after_utc <= batch_finished_utc
            )
        if not bracketed:
            raise HarnessError(
                "batch UTC evidence does not bracket snapshots and samples"
            )
        if (
            batch_finished_utc - batch_started_utc > dt.timedelta(hours=24)
            or batch_finished_utc
            > dt.datetime.now(dt.timezone.utc) + dt.timedelta(minutes=5)
        ):
            raise HarnessError("batch UTC evidence is outside the allowed window")
        if batch.get("status") == "completed":
            if (
                not has_complete_raw_samples
                or not recomputed_stability["valid"]
                or not environment_valid
            ):
                raise HarnessError("completed batch must have stable complete raw samples")
            recomputed_evaluation = evaluate_batch(
                workloads, manifest["thresholds"], True
            )
            assert_recomputed_equal(
                batch.get("evaluation"),
                recomputed_evaluation,
                f"{build_mode}.batches[{attempt_index - 1}].evaluation",
            )
            if batch.get("invalidation_reasons"):
                raise HarnessError("completed batch cannot have invalidation reasons")
            completed_count += 1
        elif batch.get("status") == "invalidated":
            invalidated_count += 1
            expected_invalidation_reasons = []
            if not environment_before["eligible"]:
                expected_invalidation_reasons.append(
                    "dynamic environment was ineligible before the batch"
                )
            if has_complete_raw_samples:
                expected_invalidation_reasons.extend(
                    recomputed_stability["issues"]
                )
            elif workloads:
                expected_invalidation_reasons.append(
                    workloads[-1]["failure_reason"]
                )
            expected_invalidation_reasons.extend(pair_issues)
            if (
                not expected_invalidation_reasons
                or batch.get("invalidation_reasons")
                != expected_invalidation_reasons
            ):
                raise HarnessError(
                    "invalidated batch reasons were not exactly recomputed "
                    "from retained evidence"
                )
            assert_recomputed_equal(
                batch.get("evaluation"),
                {"environment_eligible": False, "verdict": "ineligible"},
                f"{build_mode}.batches[{attempt_index - 1}].evaluation",
            )
            if (
                has_complete_raw_samples
                and recomputed_stability["valid"]
                and environment_valid
            ):
                raise HarnessError("stable complete raw samples cannot be invalidated")
        else:
            raise HarnessError(f"{build_mode} batch status is invalid")
        if completed_count == 2 and attempt_index != len(batches):
            raise HarnessError("samples were recorded after two accepted batches")
    if invalidated_count > 2:
        raise HarnessError(f"{build_mode} exceeded one automatic rerun")
    status = protocol.get("status")
    if status == "completed":
        if (
            len(protocol.get("correctness", [])) != 3
            or correctness_failure is not None
        ):
            raise HarnessError(
                f"{build_mode} completion requires all three correctness gates"
            )
        if completed_count != 2 or invalidated_count > 1:
            raise HarnessError(
                f"{build_mode} completion requires two independent batches "
                "and at most one retained invalid attempt"
            )
        expected_verdict = (
            "pass"
            if all(
                batch["evaluation"]["verdict"] == "pass"
                for batch in batches
                if batch["status"] == "completed"
            )
            else "fail"
        )
        if protocol.get("verdict") != expected_verdict:
            raise HarnessError(f"{build_mode} protocol verdict was not recomputed")
    elif status == "ineligible":
        if correctness_failure is not None:
            expected_reason = (
                "formal correctness gate failed: " + correctness_failure
            )
            if (
                batches
                or protocol.get("reason") != expected_reason
            ):
                raise HarnessError(
                    f"{build_mode} correctness failure did not retain the "
                    "exact failed prefix before measurement"
                )
        elif completed_count == 2 or invalidated_count != 2:
            raise HarnessError(
                f"{build_mode} ineligibility must retain the failed attempt and retry"
            )
        if protocol.get("verdict") != "not_evaluated":
            raise HarnessError(f"{build_mode} ineligible protocol cannot have a verdict")
    elif status == "unavailable":
        if batches or protocol.get("verdict") != "not_evaluated":
            raise HarnessError(f"{build_mode} unavailable protocol has measurements")
    else:
        raise HarnessError(f"{build_mode} protocol status is invalid")
    return snapshot_ids


def validate_static_authorization(
    result: Dict[str, Any],
    manifest: Dict[str, Any],
    require_eligible: bool,
    *,
    live_file: bool = True,
) -> None:
    provenance = result.get("provenance", {})
    qualification = provenance.get("environment_qualification", {})
    if qualification.get("kind") != "canonical-host-static-authorization-v1":
        raise HarnessError("canonical-host static authorization is missing")
    expected_bindings = qualification_bindings(
        provenance.get("host", {}),
        provenance.get("toolchains", {}),
        provenance.get("source_lock", []),
        result.get("release_lanes", {}),
        provenance.get("prepared_bundle_sha256"),
    )
    path_value = qualification.get("qualification_path")
    if live_file or not isinstance(path_value, str):
        derived_qualification = environment_qualification(
            manifest,
            path_value if isinstance(path_value, str) else None,
            expected_bindings,
        )
    else:
        source_record = {
            "schema": 1,
            "canonical_host_id": qualification.get("canonical_host_id"),
            "captured_at_utc": qualification.get("captured_at_utc"),
            "dynamic_policy": qualification.get("dynamic_policy"),
            "bindings": qualification.get("provided_bindings"),
            "checks": qualification.get("checks"),
        }
        canonical_source = (
            json.dumps(source_record, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        source_sha256 = v1.sha256_bytes(canonical_source)
        if source_sha256 != qualification.get("qualification_sha256"):
            raise HarnessError(
                "embedded static authorization does not reproduce its "
                "canonical qualification file SHA"
            )
        derived_qualification = derive_environment_qualification(
            manifest,
            source_record,
            path_value,
            source_sha256,
            expected_bindings,
        )
    if qualification != derived_qualification:
        raise HarnessError(
            "embedded static authorization differs from the qualification file"
        )
    if require_eligible:
        if (
            re.fullmatch(
                r"[0-9a-f]{64}",
                str(provenance.get("prepared_bundle_sha256")),
            )
            is None
            or
            qualification.get("status") != "eligible"
            or qualification.get("eligible") is not True
            or qualification.get("policy") != "fail-closed"
            or qualification.get("provided_bindings") != expected_bindings
            or qualification.get("binding_mismatches") != []
            or qualification.get("missing_or_unqualified") != []
        ):
            raise HarnessError("completed artifact lacks eligible static authorization")
        recorded_host_os = str(
            provenance.get("host", {}).get("os", "")
        )
        path_is_absolute = (
            Path(path_value).is_absolute()
            if live_file and isinstance(path_value, str)
            else artifact_path_is_absolute(path_value, recorded_host_os)
        )
        if not isinstance(path_value, str) or not path_is_absolute:
            raise HarnessError("static authorization path is not canonical")
        if live_file:
            path = Path(path_value)
            if not path.is_file() or v1.sha256_file(path) != qualification.get(
                "qualification_sha256"
            ):
                raise HarnessError(
                    "static authorization file binding is invalid"
                )
        checks = qualification.get("checks", {})
        if set(checks) != set(EXPECTED_REQUIRED_CHECKS):
            raise HarnessError("static authorization checks are incomplete")
        for check_id, check in checks.items():
            if (
                check.get("status") != "qualified"
                or check.get("value") in (None, "")
                or not check.get("source")
                or check.get("evidence", {}).get("value_sha256")
                != canonical_json_sha256(check.get("value"))
            ):
                raise HarnessError(f"static authorization check failed: {check_id}")
        expected_values = {
            "canonical_host_identity": expected_bindings[
                "authority_host_sha256"
            ],
            "toolchain_identity": expected_bindings[
                "reference_toolchains_sha256"
            ],
            "frozen_source_lock": expected_bindings[
                "frozen_source_lock_sha256"
            ],
        }
        for check_id, expected_value in expected_values.items():
            if checks[check_id]["value"] != expected_value:
                raise HarnessError(f"static authorization {check_id} is cross-bound")


def validate_embedded_compiler_tool_authority(
    compiler_build: Dict[str, Any],
    checkout: PurePath,
    cargo_environment: Dict[str, str],
    label: str,
    host_os: str,
    build_environment: Dict[str, Any],
) -> None:
    cargo = compiler_build.get("cargo", {})
    rustc = compiler_build.get("rustc", {})
    resolution = compiler_build.get("rustup_resolution", {})
    cargo_path = artifact_path(cargo.get("path"), host_os)
    rustc_path = artifact_path(rustc.get("path"), host_os)
    sysroot = artifact_path(rustc.get("sysroot"), host_os)
    for path, record, tool_label in (
        (cargo_path, cargo, "Cargo"),
        (rustc_path, rustc, "rustc"),
    ):
        if (
            not path.is_absolute()
            or not artifact_path_is_absolute(record.get("realpath"), host_os)
            or re.fullmatch(r"[0-9a-f]{64}", str(record.get("sha256")))
            is None
        ):
            raise HarnessError(
                f"{label} compiler-build {tool_label} identity is incomplete"
            )

    rustup = resolution.get("rustup", {})
    rustup_path = artifact_path(rustup.get("path"), host_os)
    invocation_path = artifact_path(
        resolution.get("invocation_path"), host_os
    )
    if (
        resolution.get("kind") != "rustup-which-v1"
        or not rustup_path.is_absolute()
        or not invocation_path.is_absolute()
        or not artifact_path_is_absolute(rustup.get("realpath"), host_os)
        or re.fullmatch(r"[0-9a-f]{64}", str(rustup.get("sha256")))
        is None
    ):
        raise HarnessError(f"{label} embedded rustup resolver authority is invalid")
    resolution_environment = {
        "CARGO_TARGET_DIR": cargo_environment["CARGO_TARGET_DIR"],
        "CARGO_HOME": cargo_environment["CARGO_HOME"],
    }
    for tool in ("cargo", "rustc"):
        command = resolution.get(f"{tool}_command", {})
        validate_command_record(command, f"{label} rustup which {tool}")
        validate_build_command_environment(
            command,
            f"{label} rustup which {tool}",
            resolution_environment,
            build_environment,
        )
        expected_argv = [str(rustup_path), "which", tool]
        if (
            command.get("argv") != expected_argv
            or command.get("command") != v1.command_text(expected_argv)
            or command.get("cwd") != str(checkout)
        ):
            raise HarnessError(
                f"{label} embedded rustup which {tool} authority is invalid"
            )

    cargo_version_command = cargo.get("version_command", {})
    validate_command_record(
        cargo_version_command, f"{label} Cargo version probe"
    )
    validate_build_command_environment(
        cargo_version_command,
        f"{label} Cargo version probe",
        cargo_environment,
        build_environment,
    )
    expected_cargo_argv = [str(cargo_path), "--version"]
    if (
        cargo_version_command.get("argv") != expected_cargo_argv
        or cargo_version_command.get("command")
        != v1.command_text(expected_cargo_argv)
        or cargo_version_command.get("cwd") != str(checkout)
        or not str(cargo.get("version_output", "")).startswith("cargo ")
    ):
        raise HarnessError(f"{label} embedded Cargo authority is invalid")

    driver_files = rustc.get("driver_files")
    expected_bin = sysroot / "bin"
    if (
        cargo_environment.get("RUSTC") != str(rustc_path)
        or rustc.get("version_fields")
        != parse_rustc_verbose_version(str(rustc.get("version_output", "")))
        or rustc.get("toolchain") != sysroot.name
        or not sysroot.is_absolute()
        or resolution.get("selected_sysroot") != str(sysroot)
        or cargo_path.parent != expected_bin
        or rustc_path.parent != expected_bin
        or not isinstance(driver_files, list)
        or not driver_files
    ):
        raise HarnessError(f"{label} embedded rustc identity is incomplete")
    seen_driver_paths: set[str] = set()
    for driver in driver_files:
        if not isinstance(driver, dict):
            raise HarnessError(
                f"{label} embedded rustc driver authority is invalid"
            )
        driver_path = artifact_path(driver.get("path"), host_os)
        if (
            set(driver) != {"path", "sha256"}
            or not driver_path.is_absolute()
            or not artifact_paths_overlap(sysroot, driver_path, host_os)
            or re.fullmatch(r"[0-9a-f]{64}", str(driver.get("sha256")))
            is None
            or str(driver_path) in seen_driver_paths
        ):
            raise HarnessError(
                f"{label} embedded rustc driver authority is invalid"
            )
        seen_driver_paths.add(str(driver_path))
    for field, suffix in (
        ("version_command", ["-vV"]),
        ("sysroot_command", ["--print", "sysroot"]),
    ):
        command = rustc.get(field, {})
        validate_command_record(command, f"{label} rustc {field}")
        validate_build_command_environment(
            command,
            f"{label} rustc {field}",
            cargo_environment,
            build_environment,
        )
        expected_argv = [str(rustc_path), *suffix]
        if (
            command.get("argv") != expected_argv
            or command.get("command") != v1.command_text(expected_argv)
            or command.get("cwd") != str(checkout)
        ):
            raise HarnessError(
                f"{label} embedded rustc {field} authority is invalid"
            )


def validate_embedded_release_lane_authority(
    result: Dict[str, Any],
) -> None:
    lanes = result.get("release_lanes", {})
    provenance = result.get("provenance", {})
    host_os = str(provenance.get("host", {}).get("os", ""))
    build_environment = provenance.get("toolchains", {}).get(
        "build_environment"
    )
    if not isinstance(build_environment, dict):
        raise HarnessError("embedded build environment authority is missing")
    if set(lanes) != {"candidate", "main"}:
        raise HarnessError("candidate/main release lanes are both required")
    if lanes["candidate"].get("expected_commit") == lanes["main"].get(
        "expected_commit"
    ):
        raise HarnessError("candidate and main commits must differ")
    if artifact_file_identity_equal(
        lanes["candidate"].get("checkout"),
        lanes["main"].get("checkout"),
        host_os,
        inspect_live_filesystem=False,
    ):
        raise HarnessError("candidate and main checkouts must differ")
    for label, lane in lanes.items():
        required = {
            "label",
            "status",
            "reason",
            "emit_c_fallback_used",
            "checkout",
            "expected_commit",
            "repository",
            "detached_head",
            "origin_url",
            "normalized_origin",
            "nomo_path",
            "nomo_sha256",
            "compiler_build",
            "capabilities",
        }
        if not required.issubset(lane) or lane.get("status") != "available":
            raise HarnessError(f"{label} release lane provenance is incomplete")
        checkout = artifact_path(lane.get("checkout"), host_os)
        nomo_path = artifact_path(lane.get("nomo_path"), host_os)
        if (
            not checkout.is_absolute()
            or not nomo_path.is_absolute()
            or lane.get("label") != label
            or lane.get("detached_head") is not True
            or lane.get("normalized_origin") != "github.com/nomo-lang/nomo"
            or normalize_nomo_origin(str(lane.get("origin_url")))
            != "github.com/nomo-lang/nomo"
            or lane.get("repository", {}).get("commit")
            != lane.get("expected_commit")
            or re.fullmatch(r"[0-9a-f]{64}", str(lane.get("nomo_sha256")))
            is None
        ):
            raise HarnessError(f"{label} release lane identity is invalid")
        compiler_build = lane["compiler_build"]
        expected_compiler_build_argv = [
            compiler_build.get("cargo", {}).get("path"),
            "build",
            "--locked",
            "--release",
            "--bin",
            "nomo",
        ]
        cargo_target_dir = str(nomo_path.parents[1])
        cargo_home = str(
            nomo_path.parents[2] / f"{label}-cargo-home"
        )
        rustc_path = compiler_build.get("rustc", {}).get("path")
        if not artifact_path_is_absolute(rustc_path, host_os):
            raise HarnessError(f"{label} compiler build lacks rustc authority")
        cargo_environment = {
            "CARGO_TARGET_DIR": cargo_target_dir,
            "CARGO_HOME": cargo_home,
            "RUSTC": rustc_path,
        }
        command = compiler_build.get("command", {})
        validate_command_record(command, f"{label} compiler self-build")
        validate_build_command_environment(
            command,
            f"{label} compiler self-build",
            cargo_environment,
            build_environment,
        )
        cargo_configs = compiler_build.get("cargo_configs")
        if not isinstance(cargo_configs, list):
            raise HarnessError(f"{label} Cargo config authority is incomplete")
        seen_configs: set[str] = set()
        for config in cargo_configs:
            if not isinstance(config, dict):
                raise HarnessError(
                    f"{label} embedded Cargo config authority is invalid"
                )
            relative = config.get("relative_path")
            config_path = artifact_path(config.get("path"), host_os)
            if (
                set(config) != {"path", "relative_path", "sha256"}
                or not isinstance(relative, str)
                or not relative
                or PurePosixPath(relative).is_absolute()
                or ".." in PurePosixPath(relative).parts
                or config_path != checkout / relative
                or re.fullmatch(r"[0-9a-f]{64}", str(config.get("sha256")))
                is None
                or relative in seen_configs
            ):
                raise HarnessError(
                    f"{label} embedded Cargo config authority is invalid"
                )
            seen_configs.add(relative)
        if (
            compiler_build.get("repository_before") != lane["repository"]
            or compiler_build.get("repository_after") != lane["repository"]
            or compiler_build.get("expected_commit")
            != lane["expected_commit"]
            or compiler_build.get("detached_head") is not True
            or compiler_build.get("origin_url") != lane["origin_url"]
            or compiler_build.get("normalized_origin")
            != "github.com/nomo-lang/nomo"
            or compiler_build.get("binary")
            != {"path": lane["nomo_path"], "sha256": lane["nomo_sha256"]}
            or command.get("argv") != expected_compiler_build_argv
            or command.get("command")
            != v1.command_text(expected_compiler_build_argv)
            or command.get("cwd") != str(checkout)
            or compiler_build.get("environment") != cargo_environment
        ):
            raise HarnessError(f"{label} compiler build is not commit-bound")
        if set(lane["capabilities"]) != set(FORMAL_BUILD_MODES) or any(
            capability.get("status") != "available"
            or capability.get("nomo_path") != lane["nomo_path"]
            or capability.get("nomo_sha256") != lane["nomo_sha256"]
            for capability in lane["capabilities"].values()
        ):
            raise HarnessError(
                f"{label} release capabilities are not compiler-bound"
            )
        for capability in lane["capabilities"].values():
            expected_help = [lane["nomo_path"], "build", "--help"]
            help_command = capability.get("help_command", {})
            validate_command_record(
                help_command, f"{label} capability probe"
            )
            validate_build_command_environment(
                help_command,
                f"{label} capability probe",
                authority_projection=build_environment,
            )
            if (
                help_command.get("argv") != expected_help
                or help_command.get("command")
                != v1.command_text(expected_help)
                or help_command.get("cwd") != str(checkout)
            ):
                raise HarnessError(
                    f"{label} capability probe command changed"
                )
        validate_embedded_compiler_tool_authority(
            compiler_build,
            checkout,
            cargo_environment,
            label,
            host_os,
            build_environment,
        )
        if label == "main" and (
            compiler_build.get("origin_main_commit")
            != lane["expected_commit"]
            or compiler_build.get("remote_main_commit")
            != lane["expected_commit"]
        ):
            raise HarnessError(
                "main embedded release lane is not bound to origin/main"
            )
    rust_identities = {
        label: compiler_toolchain_identity(lane["compiler_build"])
        for label, lane in lanes.items()
    }
    if rust_identities["candidate"] != rust_identities["main"]:
        raise HarnessError(
            "candidate and main compiler builds use different actual Rust toolchains"
        )


def validate_live_release_lane_authority(result: Dict[str, Any]) -> None:
    lanes = result.get("release_lanes", {})
    if set(lanes) != {"candidate", "main"}:
        raise HarnessError("candidate/main release lanes are both required")
    if lanes["candidate"].get("expected_commit") == lanes["main"].get(
        "expected_commit"
    ):
        raise HarnessError("candidate and main commits must differ")
    if Path(str(lanes["candidate"].get("checkout"))).resolve() == Path(
        str(lanes["main"].get("checkout"))
    ).resolve():
        raise HarnessError("candidate and main checkouts must differ")
    for label, lane in lanes.items():
        required = {
            "label",
            "status",
            "reason",
            "emit_c_fallback_used",
            "checkout",
            "expected_commit",
            "repository",
            "detached_head",
            "origin_url",
            "normalized_origin",
            "nomo_path",
            "nomo_sha256",
            "compiler_build",
            "capabilities",
        }
        if not required.issubset(lane) or lane.get("status") != "available":
            raise HarnessError(f"{label} release lane provenance is incomplete")
        if (
            lane.get("label") != label
            or lane.get("detached_head") is not True
            or lane.get("normalized_origin") != "github.com/nomo-lang/nomo"
            or normalize_nomo_origin(str(lane.get("origin_url")))
            != "github.com/nomo-lang/nomo"
            or lane.get("repository", {}).get("commit")
            != lane.get("expected_commit")
        ):
            raise HarnessError(f"{label} release lane identity is invalid")
        compiler_build = lane["compiler_build"]
        expected_compiler_build_argv = [
            compiler_build.get("cargo", {}).get("path"),
            "build",
            "--locked",
            "--release",
            "--bin",
            "nomo",
        ]
        cargo_target_dir = str(Path(lane["nomo_path"]).parents[1].resolve())
        cargo_home = str(
            (
                Path(cargo_target_dir).parent
                / f"{label}-cargo-home"
            ).resolve()
        )
        rustc_path = compiler_build.get("rustc", {}).get("path")
        if not isinstance(rustc_path, str) or not rustc_path:
            raise HarnessError(f"{label} compiler build lacks rustc authority")
        cargo_environment = {
            "CARGO_TARGET_DIR": cargo_target_dir,
            "CARGO_HOME": cargo_home,
            "RUSTC": rustc_path,
        }
        validate_command_record(
            compiler_build.get("command", {}),
            f"{label} compiler self-build",
        )
        validate_build_command_environment(
            compiler_build.get("command", {}),
            f"{label} compiler self-build",
            cargo_environment,
        )
        expected_cargo_configs = cargo_config_provenance(
            Path(lane["checkout"])
        )
        if (
            compiler_build.get("repository_before") != lane["repository"]
            or compiler_build.get("repository_after") != lane["repository"]
            or compiler_build.get("expected_commit") != lane["expected_commit"]
            or compiler_build.get("detached_head") is not True
            or compiler_build.get("origin_url") != lane["origin_url"]
            or compiler_build.get("normalized_origin")
            != "github.com/nomo-lang/nomo"
            or compiler_build.get("binary")
            != {"path": lane["nomo_path"], "sha256": lane["nomo_sha256"]}
            or compiler_build.get("command", {}).get("argv")
            != expected_compiler_build_argv
            or compiler_build.get("command", {}).get("command")
            != v1.command_text(expected_compiler_build_argv)
            or compiler_build.get("command", {}).get("cwd")
            != str(Path(lane["checkout"]).resolve())
            or compiler_build.get("environment")
            != cargo_environment
            or compiler_build.get("cargo_configs")
            != expected_cargo_configs
        ):
            raise HarnessError(f"{label} compiler build is not commit-bound")
        if set(lane["capabilities"]) != set(FORMAL_BUILD_MODES) or any(
            capability.get("status") != "available"
            or capability.get("nomo_path") != lane["nomo_path"]
            or capability.get("nomo_sha256") != lane["nomo_sha256"]
            for capability in lane["capabilities"].values()
        ):
            raise HarnessError(f"{label} release capabilities are not compiler-bound")
        for capability in lane["capabilities"].values():
            expected_help = [lane["nomo_path"], "build", "--help"]
            help_command = capability.get("help_command", {})
            validate_command_record(
                help_command, f"{label} capability probe"
            )
            validate_build_command_environment(
                help_command, f"{label} capability probe"
            )
            if help_command.get("argv") != expected_help:
                raise HarnessError(f"{label} capability probe command changed")
        validate_compiler_tool_authority(
            compiler_build,
            Path(lane["checkout"]),
            cargo_environment,
            label,
        )
        checkout = Path(lane["checkout"])
        repository_live = v1.repository_state(checkout, require_clean=True)
        if (
            repository_live != lane["repository"]
            or v1.git_capture(checkout, ["rev-parse", "--abbrev-ref", "HEAD"])
            != "HEAD"
            or normalize_nomo_origin(
                v1.git_capture(checkout, ["remote", "get-url", "origin"])
            )
            != "github.com/nomo-lang/nomo"
        ):
            raise HarnessError(f"{label} live checkout no longer matches provenance")
        if label == "main":
            origin_main = v1.git_capture(checkout, ["rev-parse", "origin/main"])
            remote_line = v1.git_capture(
                checkout, ["ls-remote", "origin", "refs/heads/main"]
            )
            remote_main = remote_line.split()[0] if remote_line.split() else None
            if (
                origin_main != lane["expected_commit"]
                or remote_main != lane["expected_commit"]
                or compiler_build.get("origin_main_commit") != origin_main
                or compiler_build.get("remote_main_commit") != remote_main
            ):
                raise HarnessError("main release lane is not current official origin/main")
    rust_identities = {
        label: compiler_toolchain_identity(lane["compiler_build"])
        for label, lane in lanes.items()
    }
    if rust_identities["candidate"] != rust_identities["main"]:
        raise HarnessError(
            "candidate and main compiler builds use different actual Rust toolchains"
        )
    validate_live_clang_authority(
        result.get("provenance", {}).get("toolchains", {})
    )
    validate_live_go_authority(
        result.get("provenance", {}).get("toolchains", {})
    )


def validate_release_lane_authority(
    result: Dict[str, Any],
    *,
    live_authority: bool = True,
) -> None:
    validate_embedded_release_lane_authority(result)
    if live_authority:
        validate_live_release_lane_authority(result)


def compiler_toolchain_identity(
    compiler_build: Dict[str, Any],
) -> Dict[str, Any]:
    cargo = compiler_build.get("cargo", {})
    rustc = compiler_build.get("rustc", {})
    return {
        "cargo": {
            key: cargo.get(key)
            for key in ("realpath", "sha256", "version_output")
        },
        "rustc": {
            key: rustc.get(key)
            for key in (
                "realpath",
                "sha256",
                "version_fields",
                "sysroot",
                "toolchain",
                "driver_files",
            )
        },
    }


def validate_compiler_tool_authority(
    compiler_build: Dict[str, Any],
    checkout: Path,
    cargo_environment: Dict[str, str],
    label: str,
) -> None:
    checkout = checkout.resolve()
    cargo = compiler_build.get("cargo", {})
    rustc = compiler_build.get("rustc", {})
    resolution = compiler_build.get("rustup_resolution", {})
    cargo_path = Path(str(cargo.get("path", "")))
    rustc_path = Path(str(rustc.get("path", "")))
    for path, record, tool_label in (
        (cargo_path, cargo, "Cargo"),
        (rustc_path, rustc, "rustc"),
    ):
        if not path.is_absolute() or not path.is_file():
            raise HarnessError(
                f"{label} compiler-build {tool_label} invocation path is unavailable"
            )
        realpath = path.resolve()
        if (
            record.get("realpath") != str(realpath)
            or record.get("sha256") != v1.sha256_file(realpath)
        ):
            raise HarnessError(
                f"{label} compiler-build {tool_label} file identity changed"
            )

    rustup = resolution.get("rustup", {})
    rustup_path = Path(str(rustup.get("path", "")))
    invocation_path = Path(str(resolution.get("invocation_path", "")))
    if (
        resolution.get("kind") != "rustup-which-v1"
        or not rustup_path.is_absolute()
        or not rustup_path.is_file()
        or rustup.get("realpath") != str(rustup_path.resolve())
        or rustup.get("sha256") != v1.sha256_file(rustup_path.resolve())
        or not invocation_path.is_absolute()
        or not invocation_path.is_file()
        or v1.sha256_file(invocation_path.resolve()) != rustup.get("sha256")
    ):
        raise HarnessError(f"{label} rustup resolver authority is invalid")
    resolution_environment = {
        "CARGO_TARGET_DIR": cargo_environment["CARGO_TARGET_DIR"],
        "CARGO_HOME": cargo_environment["CARGO_HOME"],
    }
    selected_paths = {"cargo": cargo_path, "rustc": rustc_path}
    for tool in ("cargo", "rustc"):
        command = resolution.get(f"{tool}_command", {})
        validate_command_record(command, f"{label} rustup which {tool}")
        validate_build_command_environment(
            command,
            f"{label} rustup which {tool}",
            resolution_environment,
        )
        expected_argv = [str(rustup_path), "which", tool]
        if (
            command.get("argv") != expected_argv
            or command.get("command") != v1.command_text(expected_argv)
            or command.get("cwd") != str(checkout)
        ):
            raise HarnessError(
                f"{label} rustup which {tool} authority is invalid"
            )
        _, stdout, _ = run_build_capture(
            expected_argv,
            30.0,
            cwd=checkout,
            approved_environment_overrides=resolution_environment,
        )
        if Path(stdout.decode("utf-8", errors="strict").strip()).resolve() != (
            selected_paths[tool].resolve()
        ):
            raise HarnessError(
                f"{label} live rustup selection changed for {tool}"
            )

    cargo_version_command = cargo.get("version_command", {})
    validate_command_record(
        cargo_version_command, f"{label} Cargo version probe"
    )
    validate_build_command_environment(
        cargo_version_command,
        f"{label} Cargo version probe",
        cargo_environment,
    )
    expected_cargo_argv = [str(cargo_path), "--version"]
    if (
        cargo_version_command.get("argv") != expected_cargo_argv
        or cargo_version_command.get("command")
        != v1.command_text(expected_cargo_argv)
        or cargo_version_command.get("cwd") != str(checkout)
        or not str(cargo.get("version_output", "")).startswith("cargo ")
    ):
        raise HarnessError(f"{label} Cargo version authority is invalid")

    if (
        cargo_environment.get("RUSTC") != str(rustc_path)
        or rustc.get("version_fields")
        != parse_rustc_verbose_version(str(rustc.get("version_output", "")))
        or rustc.get("toolchain")
        != Path(str(rustc.get("sysroot", ""))).name
        or not Path(str(rustc.get("sysroot", ""))).is_absolute()
        or resolution.get("selected_sysroot") != rustc.get("sysroot")
        or cargo_path.parent.resolve()
        != (Path(str(rustc.get("sysroot"))) / "bin").resolve()
        or rustc_path.parent.resolve()
        != (Path(str(rustc.get("sysroot"))) / "bin").resolve()
    ):
        raise HarnessError(f"{label} rustc identity is not compiler-build bound")
    live_driver_files = [
        {"path": str(path.resolve()), "sha256": v1.sha256_file(path)}
        for path in sorted(
            (
                path
                for path in Path(str(rustc.get("sysroot"))).rglob(
                    "*rustc_driver*"
                )
                if path.is_file()
            ),
            key=lambda path: str(path.resolve()).casefold(),
        )
    ]
    if not live_driver_files or rustc.get("driver_files") != live_driver_files:
        raise HarnessError(
            f"{label} rustc driver artifacts changed or are incomplete"
        )
    for field, suffix in (
        ("version_command", ["-vV"]),
        ("sysroot_command", ["--print", "sysroot"]),
    ):
        command = rustc.get(field, {})
        validate_command_record(command, f"{label} rustc {field}")
        validate_build_command_environment(
            command,
            f"{label} rustc {field}",
            cargo_environment,
        )
        expected_argv = [str(rustc_path), *suffix]
        if (
            command.get("argv") != expected_argv
            or command.get("command") != v1.command_text(expected_argv)
            or command.get("cwd") != str(checkout)
        ):
            raise HarnessError(f"{label} rustc {field} authority is invalid")

    live_cargo_command, live_cargo_stdout, _ = run_build_capture(
        expected_cargo_argv,
        30.0,
        cwd=checkout,
        approved_environment_overrides=cargo_environment,
    )
    if (
        live_cargo_command.get("exit_code") != 0
        or live_cargo_stdout.decode("utf-8", errors="strict").strip()
        != cargo["version_output"]
    ):
        raise HarnessError(f"{label} live Cargo version no longer matches authority")
    live_rustc = rustc_authority(
        rustc_path,
        checkout,
        cargo_environment,
    )
    stable_fields = {
        "path",
        "realpath",
        "sha256",
        "version_output",
        "version_fields",
        "sysroot",
        "toolchain",
        "driver_files",
    }
    if any(live_rustc[field] != rustc.get(field) for field in stable_fields):
        raise HarnessError(f"{label} live rustc no longer matches compiler-build authority")


def validate_live_clang_authority(toolchains: Dict[str, Any]) -> None:
    for key, driver_name in (("clang", "clang"), ("clangxx", "clang++")):
        record = toolchains.get(key, {})
        path = Path(str(record.get("path", "")))
        if not path.is_absolute() or not path.is_file():
            raise HarnessError(f"{key} invocation path is unavailable")
        live_identity = clang_selected_driver(path, driver_name)
        identity_fields = {
            "path",
            "invocation_realpath",
            "invocation_sha256",
            "selected_path",
            "realpath",
            "sha256",
        }
        if any(
            live_identity[field] != record.get(field)
            for field in identity_fields
        ):
            raise HarnessError(f"{key} selected driver identity changed")
        selection = record.get("selection_command")
        if platform.system() == "Darwin":
            validate_command_record(selection, f"{key} xcrun selection probe")
            if selection != live_identity["selection_command"]:
                stable_command_fields = {"argv", "command", "cwd", "exit_code", "environment"}
                if not isinstance(selection, dict) or any(
                    selection.get(field)
                    != live_identity["selection_command"].get(field)
                    for field in stable_command_fields
                ):
                    raise HarnessError(f"{key} xcrun selection authority changed")
            validate_build_command_environment(
                selection, f"{key} xcrun selection probe"
            )
            if selection.get("argv") != [
                "/usr/bin/xcrun",
                "--find",
                driver_name,
            ]:
                raise HarnessError(f"{key} xcrun selection argv changed")
        elif selection is not None:
            raise HarnessError(f"{key} has unexpected xcrun selection authority")

        version_command = record.get("version_command", {})
        validate_command_record(version_command, f"{key} version probe")
        validate_build_command_environment(
            version_command, f"{key} version probe"
        )
        expected_version_argv = [
            str(path),
            *CLANG_DRIVER_CONFIG_FLAGS,
            "--version",
        ]
        if version_command.get("argv") != expected_version_argv:
            raise HarnessError(f"{key} version probe argv changed")
        live_version, _ = build_probe_output(
            path, [*CLANG_DRIVER_CONFIG_FLAGS, "--version"]
        )
        if (
            live_version != record.get("version_output")
            or parse_clang_version(live_version) != record.get("version")
            or parse_clang_installation(live_version, path)
            != record.get("installation")
        ):
            raise HarnessError(f"{key} live version authority changed")

        target_command = record.get("target_command", {})
        validate_command_record(target_command, f"{key} target probe")
        validate_build_command_environment(
            target_command, f"{key} target probe"
        )
        expected_target_argv = [
            str(path),
            *CLANG_DRIVER_CONFIG_FLAGS,
            "-print-target-triple",
        ]
        if target_command.get("argv") != expected_target_argv:
            raise HarnessError(f"{key} target probe argv changed")
        live_target, _ = clang_target(path)
        if live_target != record.get("target_triple"):
            raise HarnessError(f"{key} live target authority changed")


def validate_live_go_authority(toolchains: Dict[str, Any]) -> None:
    record = toolchains.get("go", {})
    path = Path(str(record.get("path", "")))
    if (
        not path.is_absolute()
        or not path.is_file()
        or record.get("realpath") != str(path.resolve())
        or record.get("sha256") != v1.sha256_file(path.resolve())
    ):
        raise HarnessError("Go invocation identity changed")
    for field, suffix in (
        ("version_command", ["version"]),
        ("goroot_command", ["env", "GOROOT"]),
    ):
        command = record.get(field, {})
        validate_command_record(command, f"Go {field}")
        validate_build_command_environment(command, f"Go {field}")
        expected_argv = [str(path), *suffix]
        if (
            command.get("argv") != expected_argv
            or command.get("command") != v1.command_text(expected_argv)
        ):
            raise HarnessError(f"Go {field} argv changed")
    _, version_stdout, _ = run_build_capture([str(path), "version"], 30.0)
    _, goroot_stdout, _ = run_build_capture(
        [str(path), "env", "GOROOT"], 30.0
    )
    if (
        version_stdout.decode("utf-8", errors="strict").strip()
        != record.get("version_output")
        or goroot_stdout.decode("utf-8", errors="strict").strip()
        != record.get("goroot")
    ):
        raise HarnessError("live Go version or GOROOT changed")


def validate_result(
    result: Dict[str, Any],
    manifest: Optional[Dict[str, Any]] = None,
    *,
    offline_replay: bool = False,
) -> None:
    if manifest is None:
        manifest = read_json_strict(DEFAULT_MANIFEST)
    required = {
        "schema",
        "suite",
        "manifest_version",
        "mode",
        "status",
        "created_at_utc",
        "claims",
        "provenance",
        "release_lanes",
        "builds",
        "build_failures",
        "correctness",
        "protocols",
        "overall_verdict",
    }
    missing = sorted(required.difference(result))
    if missing:
        raise HarnessError("v2 result is missing fields: " + ", ".join(missing))
    if result.get("schema") != 2:
        raise HarnessError("unsupported Benchmarks Game v2 result schema")
    if result.get("suite") != "nomo-benchmarksgame-cpu-parity-v2":
        raise HarnessError("v2 result suite identity mismatch")
    status = result.get("status")
    if status not in {
        "correctness-only",
        "prepared",
        "unavailable",
        "ineligible",
        "completed",
    }:
        raise HarnessError("v2 result has an invalid status")
    claims = result.get("claims", {})
    if claims.get("scope") != "RFC 0043 frozen three-workload CPU parity suite only":
        raise HarnessError("v2 result scope changed")
    if status != "completed" and claims.get("claim_eligible") is not False:
        raise HarnessError("incomplete v2 evidence cannot be claim-eligible")
    if any(
        lane.get("emit_c_fallback_used") is not False
        for lane in result.get("release_lanes", {}).values()
    ):
        raise HarnessError("release lane fallback is forbidden")
    result_host_os = str(
        result.get("provenance", {}).get("host", {}).get("os")
    )
    if (
        not offline_replay
        and result_host_os == "Windows"
        and os.name == "nt"
    ):
        canonical_windows_build_support(refresh=True)
    validate_build_provenance(
        result, manifest, live_filesystem=not offline_replay
    )
    provenance = result.get("provenance", {})
    runtime_environments = validate_runtime_environment_authority(
        provenance.get("toolchains", {}).get("runtime_environments"),
        provenance.get("toolchains", {}).get("build_environment", {}),
        result_host_os,
    )
    recorded_suite_root = artifact_path(
        provenance.get("manifest_path"), result_host_os
    ).parent
    if provenance.get("source_lock") != frozen_source_lock(manifest):
        raise HarnessError("result source lock changed")
    if (
        provenance.get("methodology") != manifest["methodology"]
        or provenance.get("thresholds") != manifest["thresholds"]
    ):
        raise HarnessError("result methodology or thresholds changed")
    collector_id = validate_collector_descriptor(
        provenance.get("collector", {}),
        str(provenance.get("host", {}).get("os")),
    )
    mode = result.get("mode")
    if mode == "correctness":
        if result.get("build_failures"):
            if result.get("correctness") or status != "ineligible":
                raise HarnessError(
                    "preflight build failure cannot contain correctness samples"
                )
            validate_build_failure_evidence(
                result,
                manifest,
                live_filesystem=not offline_replay,
            )
        else:
            if not result.get("correctness"):
                raise HarnessError(
                    "correctness mode must retain a success or failed prefix"
                )
            correctness_failure = validate_correctness_evidence(
                result["correctness"],
                "baseline-emit-c",
                CORRECTNESS_BASELINE_LANES,
                manifest,
                result["builds"],
                collector_id,
                runtime_environments,
                result_host_os,
                recorded_suite_root,
            )
            if status == "correctness-only":
                if correctness_failure is not None:
                    raise HarnessError(
                        "correctness-only status cannot contain a failed tail"
                    )
            elif status == "ineligible":
                if correctness_failure is None:
                    raise HarnessError(
                        "ineligible correctness mode requires a failed tail"
                    )
            else:
                raise HarnessError(
                    "correctness mode status must be correctness-only or ineligible"
                )
    elif result.get("build_failures"):
        if (
            mode != "prepare"
            or status != "unavailable"
            or result.get("correctness")
        ):
            raise HarnessError(
                "formal build failure evidence requires unavailable prepare mode"
            )
        validate_build_failure_evidence(
            result,
            manifest,
            live_filesystem=not offline_replay,
        )
        if any(
            lane.get("status") != "available"
            for lane in result.get("release_lanes", {}).values()
        ):
            raise HarnessError(
                "formal build failure requires both exact compiler authorities"
            )
        validate_release_lane_authority(
            result, live_authority=not offline_replay
        )
        if (
            provenance.get("manifest_sha256") != EXPECTED_V2_MANIFEST_SHA
            or (
                not offline_replay
                and provenance.get("manifest_path")
                != str(DEFAULT_MANIFEST.resolve())
            )
        ):
            raise HarnessError(
                "formal build failure lacks canonical manifest authority"
            )
    if status in {"prepared", "unavailable", "ineligible"}:
        if result["overall_verdict"] != "not_evaluated":
            raise HarnessError("unavailable/ineligible evidence cannot have a verdict")
    protocols = result.get("protocols", {})
    if set(protocols) != set(FORMAL_BUILD_MODES):
        raise HarnessError("both formal build-mode protocols are required")
    if mode != "correctness" and status != "prepared":
        aggregate = aggregate_protocol_outcome(protocols)
        if (
            status != aggregate["status"]
            or result.get("overall_verdict")
            != aggregate["overall_verdict"]
            or claims.get("claim_eligible")
            is not aggregate["claim_eligible"]
        ):
            raise HarnessError(
                "top-level status, verdict, or claim does not match protocols"
            )
    has_formal_evidence = any(
        protocol.get("batches")
        or protocol.get("status") in {"completed", "ineligible"}
        for protocol in protocols.values()
    )
    validate_static_authorization(
        result,
        manifest,
        status == "completed" or has_formal_evidence,
        live_file=not offline_replay,
    )
    if status == "prepared" or has_formal_evidence:
        if offline_replay:
            validate_release_lane_authority(
                result, live_authority=False
            )
            validate_prepared_structure(result)
        else:
            validate_result_prepared_authority(
                result,
                require_exact_result=status == "prepared",
            )
    all_snapshot_ids: set[str] = set()
    for build_mode in FORMAL_BUILD_MODES:
        mode_snapshot_ids = validate_protocol(
            protocols[build_mode],
            build_mode,
            manifest,
            result["builds"],
            collector_id,
            provenance["environment_qualification"],
            str(provenance.get("host", {}).get("os")),
            str(provenance.get("host", {}).get("architecture")),
            runtime_environments,
            recorded_suite_root,
            live_authority=not offline_replay,
        )
        if all_snapshot_ids.intersection(mode_snapshot_ids):
            raise HarnessError("dynamic qualification snapshots were reused across modes")
        all_snapshot_ids.update(mode_snapshot_ids)
    if has_formal_evidence:
        if (
            provenance.get("manifest_sha256") != EXPECTED_V2_MANIFEST_SHA
            or (
                not offline_replay
                and provenance.get("manifest_path")
                != str(DEFAULT_MANIFEST.resolve())
            )
        ):
            raise HarnessError(
                "formal artifact lacks canonical manifest authority"
            )
        if any(
            lane.get("status") != "available"
            for lane in result.get("release_lanes", {}).values()
        ):
            raise HarnessError("formal evidence requires both self-built compilers")
        builds = result.get("builds", {})
        if set(builds) != set(WORKLOAD_IDS) or any(
            set(build.get("modes", {})) != set(FORMAL_BUILD_MODES)
            for build in builds.values()
        ):
            raise HarnessError("formal evidence requires both builds for all workloads")
    if status == "completed":
        if any(
            protocol["status"] != "completed" for protocol in protocols.values()
        ):
            raise HarnessError("both formal build modes must complete")


def default_output_path(mode: str) -> Path:
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return DEFAULT_RESULTS_ROOT / f"{mode}-{timestamp}.json"


def base_result(
    manifest: Dict[str, Any],
    manifest_path: Path,
    mode: str,
    repository: Dict[str, Any],
    toolchains: Dict[str, Any],
    collector: ProcessCollector,
    qualification: Dict[str, Any],
    release_lanes: Dict[str, Any],
    host: Dict[str, Any],
) -> Dict[str, Any]:
    return {
        "schema": 2,
        "suite": manifest["suite"],
        "manifest_version": manifest["manifest_version"],
        "mode": mode,
        "status": "unavailable",
        "created_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "claims": {
            "claim_eligible": False,
            "scope": "RFC 0043 frozen three-workload CPU parity suite only",
            "limitations": [
                "No async, I/O, concurrency, whole-language, or general C/C++ claim.",
                "Go and semantic-C are diagnostic and never decide parity.",
            ],
        },
        "provenance": {
            "repository": repository,
            "manifest_path": str(manifest_path),
            "manifest_sha256": v1.sha256_file(manifest_path),
            "predecessor": manifest["predecessor"],
            "rfc": manifest["rfc"],
            "host": host,
            "toolchains": toolchains,
            "collector": collector.descriptor(),
            "methodology": manifest["methodology"],
            "thresholds": manifest["thresholds"],
            "environment_qualification": qualification,
            "source_lock": frozen_source_lock(manifest),
            "prepared_bundle_sha256": None,
            "prepared_bundle_path": None,
            "qualification_request_path": None,
        },
        "release_lanes": release_lanes,
        "builds": {},
        "build_failures": [],
        "correctness": [],
        "protocols": {
            build_mode: {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": "formal measurement was not run",
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }
            for build_mode in FORMAL_BUILD_MODES
        },
        "overall_verdict": "not_evaluated",
    }


def run_correctness(
    arguments: argparse.Namespace,
    manifest: Dict[str, Any],
    manifest_path: Path,
    suite_root: Path,
    output_path: Path,
    repository: Dict[str, Any],
    toolchains: Dict[str, Any],
    collector: ProcessCollector,
    host: Dict[str, Any],
) -> Dict[str, Any]:
    current_capability = release_capability(Path(toolchains["nomo"]["path"]), "current")
    release_lanes = {
        "candidate": {
            "label": "candidate",
            "status": "unavailable",
            "reason": "formal candidate was not supplied in correctness-only mode",
            "emit_c_fallback_used": False,
        },
        "main": {
            **current_capability,
            "label": "main",
        },
    }
    qualification = environment_qualification(
        manifest,
        None,
        qualification_bindings(
            host, toolchains, frozen_source_lock(manifest), release_lanes
        ),
    )
    result = base_result(
        manifest,
        manifest_path,
        "correctness",
        repository,
        toolchains,
        collector,
        qualification,
        release_lanes,
        host,
    )
    bundle_root = output_path.with_suffix("")
    binaries_by_workload = {}
    for workload in manifest["workloads"]:
        try:
            reference_build, binaries = build_reference_workload(
                workload,
                suite_root,
                bundle_root,
                toolchains,
                float(manifest["methodology"]["build_timeout_seconds"]),
                include_nomo_baseline=True,
            )
        except WorkloadBuildError as error:
            result["build_failures"].append(error.record)
            result["status"] = "ineligible"
            return result
        result["builds"][workload["id"]] = {
            "references": reference_build,
            "modes": {},
        }
        binaries_by_workload[workload["id"]] = binaries
    result["correctness"] = correctness_gate(
        manifest,
        suite_root,
        binaries_by_workload,
        CORRECTNESS_BASELINE_LANES,
        collector,
        "baseline-emit-c",
    )
    result["status"] = (
        "correctness-only"
        if (
            len(result["correctness"]) == len(WORKLOAD_IDS)
            and all(
                item.get("status") == "completed"
                for item in result["correctness"]
            )
        )
        else "ineligible"
    )
    return result


def map_bundle_paths(value: Any, bundle_root: Path, tokenize: bool) -> Any:
    if isinstance(value, dict):
        return {
            key: map_bundle_paths(item, bundle_root, tokenize)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [
            map_bundle_paths(item, bundle_root, tokenize) for item in value
        ]
    if isinstance(value, str):
        root = str(bundle_root.resolve())
        return (
            value.replace(root, "${BUNDLE}")
            if tokenize
            else value.replace("${BUNDLE}", root)
        )
    return value


def prepared_file_inventory(bundle_root: Path) -> list[Dict[str, Any]]:
    excluded = {"prepared-bundle.json", "qualification-request.json"}
    inventory = []
    for path in sorted(bundle_root.rglob("*")):
        metadata = lexical_path_lstat(path)
        if metadata is None:
            raise HarnessError(
                f"prepared bundle entry disappeared during inventory: {path}"
            )
        if lexical_path_is_link(path, metadata):
            raise HarnessError(
                "prepared bundle cannot contain symlinks or junctions"
            )
        relative = path.relative_to(bundle_root).as_posix()
        if stat.S_ISDIR(metadata.st_mode) or relative in excluded:
            continue
        if not stat.S_ISREG(metadata.st_mode):
            raise HarnessError(
                f"prepared bundle payload is not a regular file: {relative}"
            )
        if int(metadata.st_nlink) != 1:
            raise HarnessError(
                f"prepared bundle payload has multiple hard links: {relative}"
            )
        inventory.append(
            {
                "path": relative,
                "sha256": v1.sha256_file(path),
                "lstat_type": "regular",
                "link_count": 1,
            }
        )
    return inventory


def canonical_prepared_bundle_root(bundle_root: Path) -> Path:
    lexical = lexical_absolute_path(bundle_root)
    metadata = lexical_path_lstat(lexical)
    if (
        metadata is None
        or lexical_path_is_link(lexical, metadata)
        or not stat.S_ISDIR(metadata.st_mode)
    ):
        raise HarnessError(
            f"prepared bundle root is not a lexical directory: {lexical}"
        )
    return lexical.resolve()


def validate_prepared_descendant(
    path: Path,
    root: Path,
    label: str,
    *,
    final_kind: str,
) -> Path:
    lexical = lexical_absolute_path(path)
    try:
        relative = lexical.relative_to(root)
    except ValueError as error:
        raise HarnessError(f"{label} points outside the prepared bundle") from error
    if not relative.parts:
        raise HarnessError(f"{label} aliases the prepared bundle root")
    for candidate in (lexical, *lexical.parents):
        metadata = lexical_path_lstat(candidate)
        if metadata is None:
            raise HarnessError(f"{label} lexical path is missing: {candidate}")
        if lexical_path_is_link(candidate, metadata):
            raise HarnessError(
                f"{label} contains a symlink or junction: {candidate}"
            )
        if candidate == lexical:
            expected_kind = (
                stat.S_ISREG(metadata.st_mode)
                if final_kind == "regular"
                else stat.S_ISDIR(metadata.st_mode)
            )
            if not expected_kind:
                raise HarnessError(
                    f"{label} is not a {final_kind} path: {candidate}"
                )
        elif not stat.S_ISDIR(metadata.st_mode):
            raise HarnessError(
                f"{label} has a non-directory lexical parent: {candidate}"
            )
        if candidate == root:
            break
    else:
        raise HarnessError(f"{label} does not descend from the prepared bundle")
    return lexical


def prepared_result_projection(
    result: Dict[str, Any], bundle_root: Path
) -> Dict[str, Any]:
    projection = json.loads(json.dumps(result))
    projection["provenance"]["prepared_bundle_sha256"] = None
    projection["provenance"]["environment_qualification"] = None
    return map_bundle_paths(projection, bundle_root, True)


def prepared_authority_projection(result: Dict[str, Any]) -> Dict[str, Any]:
    provenance = result.get("provenance", {})
    claims = result.get("claims", {})
    return {
        "schema": result.get("schema"),
        "suite": result.get("suite"),
        "manifest_version": result.get("manifest_version"),
        "claims": {
            "scope": claims.get("scope"),
            "limitations": claims.get("limitations"),
        },
        "provenance": {
            key: provenance.get(key)
            for key in (
                "repository",
                "manifest_path",
                "manifest_sha256",
                "predecessor",
                "rfc",
                "host",
                "toolchains",
                "methodology",
                "thresholds",
                "source_lock",
                "prepared_bundle_sha256",
                "prepared_bundle_path",
                "qualification_request_path",
            )
        },
        "release_lanes": result.get("release_lanes"),
        "builds": result.get("builds"),
    }


def canonical_qualification_request(result: Dict[str, Any]) -> Dict[str, Any]:
    provenance = result.get("provenance", {})
    digest = provenance.get("prepared_bundle_sha256")
    return {
        "schema": 1,
        "kind": "benchmarksgame-v2-qualification-request",
        "bundle_sha256": digest,
        "bindings": qualification_bindings(
            provenance.get("host", {}),
            provenance.get("toolchains", {}),
            provenance.get("source_lock", []),
            result.get("release_lanes", {}),
            digest,
        ),
        "dynamic_policy": DYNAMIC_ENVIRONMENT_POLICY,
        "required_checks": list(EXPECTED_REQUIRED_CHECKS),
    }


def canonical_qualification_request_contract(
    result: Dict[str, Any],
) -> Dict[str, Any]:
    request = canonical_qualification_request(result)
    request["bundle_sha256"] = None
    request["bindings"].pop("prepared_bundle_sha256", None)
    return request


def validate_prepared_structure(result: Dict[str, Any]) -> None:
    release_lanes = result.get("release_lanes", {})
    if set(release_lanes) != {"candidate", "main"} or any(
        release_lanes[lane].get("status") != "available"
        or set(release_lanes[lane].get("capabilities", {}))
        != set(FORMAL_BUILD_MODES)
        or any(
            release_lanes[lane]["capabilities"][mode].get("status")
            != "available"
            for mode in FORMAL_BUILD_MODES
        )
        for lane in ("candidate", "main")
    ):
        raise HarnessError(
            "prepared bundle requires both available exact compiler lanes "
            "and both formal capabilities"
        )
    builds = result.get("builds", {})
    if set(builds) != set(WORKLOAD_IDS):
        raise HarnessError(
            "prepared bundle must contain exactly the three frozen workloads"
        )
    for workload_id in WORKLOAD_IDS:
        build = builds[workload_id]
        references = build.get("references", {})
        if (
            set(references.get("binaries", {})) != set(REFERENCE_LANES)
            or set(references.get("compiled_sources", {}))
            != set(REFERENCE_LANES)
            or set(build.get("modes", {})) != set(FORMAL_BUILD_MODES)
        ):
            raise HarnessError(
                f"{workload_id} prepared references or formal modes are incomplete"
            )
        for build_mode in FORMAL_BUILD_MODES:
            if set(build["modes"][build_mode]) != {"candidate", "main"}:
                raise HarnessError(
                    f"{workload_id} {build_mode} prepared lanes are incomplete"
                )


def validate_prepared_bundle_files(
    result: Dict[str, Any],
    bundle_root: Path,
    inventory: Sequence[Dict[str, Any]],
) -> None:
    root = canonical_prepared_bundle_root(bundle_root)
    root_resolved = root
    inventory_by_path = {
        item["path"]: item for item in inventory
    }

    def require_bundle_directory(path_value: Any, label: str) -> Path:
        if not isinstance(path_value, str):
            raise HarnessError(f"{label} prepared directory identity is incomplete")
        lexical = validate_prepared_descendant(
            Path(path_value), root, label, final_kind="directory"
        )
        return lexical.resolve()

    def require_bundle_file(record: Dict[str, Any], label: str) -> Path:
        path_value = record.get("path")
        expected_sha = record.get("sha256")
        if (
            not isinstance(path_value, str)
            or re.fullmatch(r"[0-9a-f]{64}", str(expected_sha)) is None
        ):
            raise HarnessError(f"{label} prepared file identity is incomplete")
        lexical = validate_prepared_descendant(
            Path(path_value), root, label, final_kind="regular"
        )
        metadata = lexical.lstat()
        if int(metadata.st_nlink) != 1:
            raise HarnessError(f"{label} has multiple hard links")
        path = lexical.resolve()
        try:
            relative = path.relative_to(root_resolved).as_posix()
        except ValueError as error:
            raise HarnessError(
                f"{label} points outside the prepared bundle"
            ) from error
        expected_inventory = {
            "path": relative,
            "sha256": expected_sha,
            "lstat_type": "regular",
            "link_count": 1,
        }
        if (
            not path.is_file()
            or v1.sha256_file(path) != expected_sha
            or inventory_by_path.get(relative) != expected_inventory
        ):
            raise HarnessError(
                f"{label} does not match the prepared inventory and live file"
            )
        return path

    for lane in ("candidate", "main"):
        require_bundle_file(
            {
                "path": result["release_lanes"][lane].get("nomo_path"),
                "sha256": result["release_lanes"][lane].get("nomo_sha256"),
            },
            f"{lane} compiler",
        )
    for workload_id in WORKLOAD_IDS:
        build = result["builds"][workload_id]
        references = build["references"]
        decisive_artifacts: list[Tuple[str, Path]] = []
        for lane in REFERENCE_LANES:
            require_bundle_file(
                references["compiled_sources"][lane],
                f"{workload_id} {lane} copied source",
            )
            decisive_artifacts.append(
                (
                    f"{lane} reference binary",
                    require_bundle_file(
                        references["binaries"][lane],
                        f"{workload_id} {lane} reference binary",
                    ),
                )
            )
        for build_mode in FORMAL_BUILD_MODES:
            for lane in ("candidate", "main"):
                formal = build["modes"][build_mode][lane]
                project = formal["project"]
                require_bundle_directory(
                    project["path"],
                    f"{workload_id} {build_mode} {lane} project",
                )
                require_bundle_file(
                    project["copied_source"],
                    f"{workload_id} {build_mode} {lane} copied Nomo source",
                )
                require_bundle_file(
                    project["copied_project_manifest"],
                    f"{workload_id} {build_mode} {lane} copied Nomo manifest",
                )
                decisive_artifacts.append(
                    (
                        f"{build_mode} {lane} binary",
                        require_bundle_file(
                            formal["binary"],
                            f"{workload_id} {build_mode} {lane} binary",
                        ),
                    )
                )
                decisive_artifacts.append(
                    (
                        f"{build_mode} {lane} generated C",
                        require_bundle_file(
                            formal["generated_c"],
                            f"{workload_id} {build_mode} {lane} generated C",
                        ),
                    )
                )
                if build_mode == "release":
                    require_bundle_file(
                        {
                            "path": formal["backend_provenance_path"],
                            "sha256": formal["backend_provenance_sha256"],
                        },
                        f"{workload_id} {lane} backend provenance",
                    )
                    sidecar = read_json_strict(
                        Path(formal["backend_provenance_path"])
                    )
                    if sidecar != formal["backend_provenance"]:
                        raise HarnessError(
                            f"{workload_id} {lane} embedded release backend "
                            "differs from its hashed sidecar"
                        )
                    for index, object_file in enumerate(
                        formal["backend_provenance"]["objects"]
                    ):
                        decisive_artifacts.append(
                            (
                                f"release {lane} backend object {index}",
                                require_bundle_file(
                                    object_file,
                                    f"{workload_id} {lane} backend object {index}",
                                ),
                            )
                        )
        for index, (left_label, left_path) in enumerate(decisive_artifacts):
            for right_label, right_path in decisive_artifacts[index + 1 :]:
                try:
                    reused = os.path.samefile(left_path, right_path)
                except OSError as error:
                    raise HarnessError(
                        f"{workload_id} cannot verify prepared artifact identity: "
                        f"{error}"
                    ) from error
                if reused:
                    raise HarnessError(
                        f"{workload_id} prepared artifact identity was reused: "
                        f"{left_label}, {right_label}"
                    )


def validate_prepared_bundle_authority(
    result: Dict[str, Any],
    bundle_root: Path,
    *,
    require_exact_result: bool,
) -> Dict[str, Any]:
    root = canonical_prepared_bundle_root(bundle_root)
    loaded = load_prepared_bundle(root)
    if require_exact_result:
        if loaded != result:
            raise HarnessError(
                "prepared result differs from the canonical bundled result"
            )
    elif prepared_authority_projection(loaded) != (
        prepared_authority_projection(result)
    ):
        raise HarnessError(
            "measured result differs from its prepared authority structure"
        )
    validate_prepared_structure(loaded)
    request_path = root / "qualification-request.json"
    if not request_path.is_file():
        raise HarnessError("prepared qualification request is missing")
    request = read_json_strict(request_path)
    expected_request = canonical_qualification_request(loaded)
    if request != expected_request:
        raise HarnessError(
            "prepared qualification request differs from the canonical request"
        )
    validate_prepared_bundle_files(
        loaded, root, prepared_file_inventory(root)
    )
    return loaded


def validate_result_prepared_authority(
    result: Dict[str, Any],
    *,
    require_exact_result: bool,
    live_authority: bool = True,
) -> None:
    provenance = result.get("provenance", {})
    prepared_path = provenance.get("prepared_bundle_path")
    request_path = provenance.get("qualification_request_path")
    if (
        not isinstance(prepared_path, str)
        or not isinstance(request_path, str)
        or not Path(prepared_path).is_absolute()
        or not Path(request_path).is_absolute()
        or not Path(prepared_path).is_file()
        or not Path(request_path).is_file()
    ):
        raise HarnessError(
            "prepared result lacks bundle and qualification request files"
        )
    validate_prepared_bundle_authority(
        result,
        Path(prepared_path).parent,
        require_exact_result=require_exact_result,
    )
    if live_authority:
        validate_release_lane_authority(result)


def prepared_bundle_digest(
    result: Dict[str, Any],
    bundle_root: Path,
    inventory: Sequence[Dict[str, Any]],
    prepared_at_utc: str,
) -> str:
    parse_utc_timestamp(prepared_at_utc)
    return canonical_json_sha256(
        {
            "schema": 1,
            "prepared_at_utc": prepared_at_utc,
            "prepared_result": prepared_result_projection(
                result, bundle_root
            ),
            "files": list(inventory),
            "qualification_request_contract": (
                canonical_qualification_request_contract(result)
            ),
        }
    )


def write_prepared_bundle(
    result: Dict[str, Any],
    bundle_root: Path,
    manifest: Dict[str, Any],
) -> Dict[str, Any]:
    result["provenance"]["prepared_bundle_path"] = str(
        (bundle_root / "prepared-bundle.json").resolve()
    )
    result["provenance"]["qualification_request_path"] = str(
        (bundle_root / "qualification-request.json").resolve()
    )
    result["status"] = "prepared"
    inventory = prepared_file_inventory(bundle_root)
    prepared_at_utc = dt.datetime.now(dt.timezone.utc).isoformat()
    digest = prepared_bundle_digest(
        result, bundle_root, inventory, prepared_at_utc
    )
    result["provenance"]["prepared_bundle_sha256"] = digest
    bindings = qualification_bindings(
        result["provenance"]["host"],
        result["provenance"]["toolchains"],
        result["provenance"]["source_lock"],
        result["release_lanes"],
        digest,
    )
    result["provenance"]["environment_qualification"] = (
        environment_qualification(manifest, None, bindings)
    )
    metadata = {
        "schema": 1,
        "bundle_sha256": digest,
        "prepared_at_utc": prepared_at_utc,
        "files": inventory,
        "prepared_result": map_bundle_paths(result, bundle_root, True),
    }
    request = canonical_qualification_request(result)
    write_canonical_json(bundle_root / "prepared-bundle.json", metadata)
    write_canonical_json(bundle_root / "qualification-request.json", request)
    return result


def load_prepared_bundle(bundle_root: Path) -> Dict[str, Any]:
    root = canonical_prepared_bundle_root(bundle_root)
    metadata_path = root / "prepared-bundle.json"
    if not metadata_path.is_file():
        raise HarnessError("prepared bundle metadata is missing")
    metadata = read_json_strict(metadata_path)
    if metadata_path.read_bytes() != (
        json.dumps(metadata, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8"):
        raise HarnessError(
            "prepared bundle metadata is not canonical byte-for-byte JSON"
        )
    expected_metadata_keys = {
        "schema",
        "bundle_sha256",
        "prepared_at_utc",
        "files",
        "prepared_result",
    }
    if (
        set(metadata) != expected_metadata_keys
        or metadata.get("schema") != 1
        or re.fullmatch(
            r"[0-9a-f]{64}", str(metadata.get("bundle_sha256"))
        )
        is None
        or not isinstance(metadata.get("files"), list)
        or not isinstance(metadata.get("prepared_result"), dict)
    ):
        raise HarnessError("prepared bundle metadata envelope changed")
    parse_utc_timestamp(metadata.get("prepared_at_utc"))
    stored_inventory = metadata.get("files")
    if any(
        not isinstance(item, dict)
        or set(item) != {
            "path",
            "sha256",
            "lstat_type",
            "link_count",
        }
        or not isinstance(item.get("path"), str)
        or not item["path"]
        or Path(item["path"]).is_absolute()
        or ".." in Path(item["path"]).parts
        or re.fullmatch(r"[0-9a-f]{64}", str(item.get("sha256"))) is None
        or item.get("lstat_type") != "regular"
        or item.get("link_count") != 1
        for item in stored_inventory
    ):
        raise HarnessError("prepared bundle inventory envelope changed")
    actual_inventory = prepared_file_inventory(root)
    if stored_inventory != actual_inventory:
        raise HarnessError("prepared bundle files were added, removed, or changed")
    tokenized_result = metadata.get("prepared_result")
    if not isinstance(tokenized_result, dict):
        raise HarnessError("prepared bundle result is missing")
    result = map_bundle_paths(tokenized_result, root, False)
    recomputed = prepared_bundle_digest(
        result,
        root,
        actual_inventory,
        metadata["prepared_at_utc"],
    )
    if (
        metadata.get("bundle_sha256") != recomputed
        or result.get("provenance", {}).get("prepared_bundle_sha256")
        != recomputed
    ):
        raise HarnessError("prepared bundle digest is invalid")
    request_path = root / "qualification-request.json"
    request = read_json_strict(request_path)
    if request_path.read_bytes() != (
        json.dumps(request, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8"):
        raise HarnessError(
            "prepared qualification request is not canonical byte-for-byte JSON"
        )
    return result


def validate_output_outside_bundle(
    output_path: Path, bundle_root: Path
) -> None:
    output = lexical_absolute_path(output_path)
    root = lexical_absolute_path(bundle_root)
    if filesystem_paths_overlap(output, root):
        raise HarnessError(
            "result output must be outside the prepared bundle"
        )


def require_new_evidence_destination(
    output_path: Path,
    mode: str,
    prepared_bundle: Optional[str],
) -> None:
    output = lexical_absolute_path(output_path)
    if output.suffix != ".json":
        raise HarnessError("result --output must use the exact .json extension")
    log_path = output.with_suffix(".log")
    created_targets = [
        (output, "result output"),
        (log_path, "result evidence log"),
    ]
    bundle_root = None
    prepared_children: Tuple[Tuple[Path, str], ...] = ()
    if mode == "correctness":
        bundle_root = output.with_suffix("")
        created_targets.append(
            (bundle_root, "correctness build bundle")
        )
    elif mode == "prepare":
        bundle_root = (
            lexical_absolute_path(Path(prepared_bundle))
            if prepared_bundle
            else output.with_suffix("")
        )
        created_targets.append((bundle_root, "prepared build bundle"))
        prepared_children = (
            (bundle_root / "prepared-bundle.json", "prepared metadata"),
            (
                bundle_root / "qualification-request.json",
                "qualification request",
            ),
        )
        created_targets.extend(prepared_children)
    elif mode == "measure":
        if not prepared_bundle:
            raise HarnessError("measure requires --prepared-bundle")
        bundle_root = lexical_absolute_path(Path(prepared_bundle))

    for target, label in created_targets:
        validate_lexical_path_chain(target, label, final_kind="absent")

    prepared_inputs: Tuple[Tuple[Path, str], ...] = ()
    if mode == "measure" and bundle_root is not None:
        prepared_inputs = (
            (bundle_root, "prepared build bundle"),
            (bundle_root / "prepared-bundle.json", "prepared metadata"),
            (
                bundle_root / "qualification-request.json",
                "qualification request",
            ),
        )
        validate_lexical_path_chain(
            prepared_inputs[0][0],
            prepared_inputs[0][1],
            final_kind="directory",
        )
        for target, label in prepared_inputs[1:]:
            validate_lexical_path_chain(target, label, final_kind="regular")

    independent_targets = list(created_targets[:2])
    if bundle_root is not None:
        independent_targets.append(
            (
                bundle_root,
                (
                    "prepared build bundle"
                    if mode in {"prepare", "measure"}
                    else "correctness build bundle"
                ),
            )
        )
    for index, (left, left_label) in enumerate(independent_targets):
        for right, right_label in independent_targets[index + 1 :]:
            if filesystem_paths_overlap(left, right):
                raise HarnessError(
                    f"preflight path collision between {left_label} "
                    f"({left}) and {right_label} ({right})"
                )

    if prepared_children:
        if filesystem_paths_overlap(
            prepared_children[0][0], prepared_children[1][0]
        ):
            raise HarnessError(
                "prepared metadata and qualification request collide"
            )
        for child, child_label in prepared_children:
            for external, external_label in created_targets[:2]:
                if filesystem_paths_overlap(child, external):
                    raise HarnessError(
                        f"preflight path collision between {child_label} "
                        f"({child}) and {external_label} ({external})"
                    )

    for child, child_label in prepared_inputs[1:]:
        for external, external_label in created_targets[:2]:
            if filesystem_paths_overlap(child, external):
                raise HarnessError(
                    f"preflight path collision between {child_label} "
                    f"({child}) and {external_label} ({external})"
                )


def write_result_exclusive(path: Path, result: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("x", encoding="utf-8", newline="\n") as output:
            output.write(json.dumps(result, indent=2, sort_keys=True) + "\n")
    except FileExistsError as error:
        raise HarnessError(
            f"result output appeared during the run; refusing to overwrite {path}"
        ) from error


def run_prepare(
    arguments: argparse.Namespace,
    manifest: Dict[str, Any],
    manifest_path: Path,
    suite_root: Path,
    output_path: Path,
    repository: Dict[str, Any],
    toolchains: Dict[str, Any],
    collector: ProcessCollector,
    host: Dict[str, Any],
) -> Dict[str, Any]:
    bundle_root = (
        lexical_absolute_path(Path(arguments.prepared_bundle))
        if arguments.prepared_bundle
        else output_path.with_suffix("")
    )
    validate_output_outside_bundle(output_path, bundle_root)
    build_timeout_seconds = float(manifest["methodology"]["build_timeout_seconds"])
    duplicate_reason = lane_pair_conflict(arguments)
    if duplicate_reason is not None:
        release_lanes = {
            lane: {
                "label": lane,
                "status": "unavailable",
                "reason": duplicate_reason,
                "emit_c_fallback_used": False,
            }
            for lane in ("candidate", "main")
        }
    else:
        release_lanes = {
            "candidate": release_lane_state(
                arguments.candidate_checkout,
                arguments.candidate_commit,
                "candidate",
                bundle_root,
                build_timeout_seconds,
                arguments.cargo,
                False,
            ),
            "main": release_lane_state(
                arguments.main_checkout,
                arguments.main_commit,
                "main",
                bundle_root,
                build_timeout_seconds,
                arguments.cargo,
                True,
            ),
        }
    qualification = environment_qualification(
        manifest,
        None,
        qualification_bindings(
            host, toolchains, frozen_source_lock(manifest), release_lanes
        ),
    )
    result = base_result(
        manifest,
        manifest_path,
        "prepare",
        repository,
        toolchains,
        collector,
        qualification,
        release_lanes,
        host,
    )
    if any(lane["status"] != "available" for lane in release_lanes.values()):
        result["status"] = "unavailable"
        return result
    mode_availability = {
        build_mode: all(
            release_lanes[lane]
            .get("capabilities", {})
            .get(build_mode, {})
            .get("status")
            == "available"
            for lane in ("candidate", "main")
        )
        for build_mode in FORMAL_BUILD_MODES
    }
    if not all(mode_availability.values()):
        for build_mode in FORMAL_BUILD_MODES:
            if mode_availability[build_mode]:
                continue
            unavailable_lanes = [
                lane
                for lane in ("candidate", "main")
                if release_lanes[lane]
                .get("capabilities", {})
                .get(build_mode, {})
                .get("status")
                != "available"
            ]
            result["protocols"][build_mode] = {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": (
                    f"{build_mode} capability unavailable for "
                    + ", ".join(unavailable_lanes)
                ),
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }
        result["status"] = "unavailable"
        return result
    if Path(release_lanes["candidate"]["checkout"]) == Path(
        release_lanes["main"]["checkout"]
    ):
        raise HarnessError("candidate and main must use separate clean checkouts")

    binaries_by_mode: Dict[str, Dict[str, Dict[str, Path]]] = {
        build_mode: {} for build_mode in FORMAL_BUILD_MODES
    }
    for workload in manifest["workloads"]:
        try:
            reference_build, reference_binaries = build_reference_workload(
                workload,
                suite_root,
                bundle_root,
                toolchains,
                float(manifest["methodology"]["build_timeout_seconds"]),
                include_nomo_baseline=False,
            )
        except WorkloadBuildError as error:
            result["build_failures"].append(error.record)
            result["status"] = "unavailable"
            return result
        build_record: Dict[str, Any] = {"references": reference_build, "modes": {}}
        for build_mode in FORMAL_BUILD_MODES:
            if not mode_availability[build_mode]:
                continue
            binaries = dict(reference_binaries)
            mode_builds = {}
            for lane in ("candidate", "main"):
                try:
                    if build_mode == "release":
                        formal_build, binary = build_release_lane(
                            workload,
                            suite_root,
                            bundle_root,
                            lane,
                            release_lanes[lane],
                            toolchains,
                            build_timeout_seconds,
                        )
                    else:
                        formal_build, binary = build_emit_c_lane(
                            workload,
                            suite_root,
                            bundle_root,
                            lane,
                            release_lanes[lane],
                            toolchains,
                            build_timeout_seconds,
                        )
                except WorkloadBuildError as error:
                    result["build_failures"].append(error.record)
                    result["status"] = "unavailable"
                    return result
                mode_builds[lane] = formal_build
                binaries[lane] = binary
            build_record["modes"][build_mode] = mode_builds
            binaries_by_mode[build_mode][workload["id"]] = binaries
        result["builds"][workload["id"]] = build_record
    return write_prepared_bundle(result, bundle_root, manifest)


def run_measurement(
    arguments: argparse.Namespace,
    manifest: Dict[str, Any],
    manifest_path: Path,
    suite_root: Path,
    output_path: Path,
    repository: Dict[str, Any],
    toolchains: Dict[str, Any],
    collector: ProcessCollector,
    host: Dict[str, Any],
) -> Dict[str, Any]:
    if not arguments.prepared_bundle:
        raise HarnessError("measure requires --prepared-bundle")
    bundle_root = lexical_absolute_path(Path(arguments.prepared_bundle))
    validate_output_outside_bundle(output_path, bundle_root)
    result = load_prepared_bundle(bundle_root)
    validate_result_schema(
        result, suite_root / "schema" / "result-v2.schema.json"
    )
    validate_result(result, manifest)
    provenance = result.get("provenance", {})
    if (
        provenance.get("manifest_path") != str(manifest_path)
        or provenance.get("manifest_sha256") != EXPECTED_V2_MANIFEST_SHA
        or provenance.get("source_lock") != frozen_source_lock(manifest)
    ):
        raise HarnessError("prepared bundle is not bound to the canonical suite")
    if provenance.get("host") != host:
        raise HarnessError("prepared bundle belongs to a different canonical host")
    if stable_toolchain_identity(provenance.get("toolchains", {})) != (
        stable_toolchain_identity(toolchains)
    ):
        raise HarnessError("prepared bundle toolchain identity changed")
    if provenance.get("repository") != repository:
        raise HarnessError("prepared bundle authority repository changed")
    digest = provenance.get("prepared_bundle_sha256")
    bindings = qualification_bindings(
        host,
        toolchains,
        provenance["source_lock"],
        result["release_lanes"],
        digest,
    )
    qualification = environment_qualification(
        manifest,
        arguments.environment_qualification,
        bindings,
    )
    if not qualification["eligible"]:
        raise HarnessError(
            "prepared bundle lacks an eligible, bundle-bound authority approval"
        )
    result["mode"] = "measure"
    result["created_at_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    result["provenance"]["collector"] = collector.descriptor()
    result["provenance"]["environment_qualification"] = qualification
    result["correctness"] = []
    binaries_by_mode: Dict[str, Dict[str, Dict[str, Path]]] = {
        build_mode: {} for build_mode in FORMAL_BUILD_MODES
    }
    for workload in manifest["workloads"]:
        workload_id = workload["id"]
        build = result["builds"][workload_id]
        reference_binaries = {
            lane: Path(build["references"]["binaries"][lane]["path"])
            for lane in REFERENCE_LANES
        }
        for build_mode in FORMAL_BUILD_MODES:
            binaries = dict(reference_binaries)
            for lane in ("candidate", "main"):
                binaries[lane] = Path(
                    build["modes"][build_mode][lane]["binary"]["path"]
                )
            binaries_by_mode[build_mode][workload_id] = binaries
    for build_mode in FORMAL_BUILD_MODES:
        result["protocols"][build_mode] = protocol_result(
            manifest,
            suite_root,
            binaries_by_mode[build_mode],
            collector,
            build_mode,
            qualification,
        )
    aggregate = aggregate_protocol_outcome(result["protocols"])
    result["status"] = aggregate["status"]
    result["overall_verdict"] = aggregate["overall_verdict"]
    result["claims"]["claim_eligible"] = aggregate["claim_eligible"]
    return result


def lane_pair_conflict(arguments: argparse.Namespace) -> Optional[str]:
    if arguments.candidate_commit and arguments.main_commit:
        if arguments.candidate_commit == arguments.main_commit:
            return "candidate and main commits must be different"
    if arguments.candidate_checkout and arguments.main_checkout:
        if Path(arguments.candidate_checkout).resolve() == Path(
            arguments.main_checkout
        ).resolve():
            return "candidate and main must use separate checkouts"
    return None


def run_suite(arguments: argparse.Namespace) -> Tuple[Path, Dict[str, Any]]:
    manifest_path = Path(arguments.manifest).resolve()
    if arguments.mode in {"prepare", "measure"}:
        if manifest_path != DEFAULT_MANIFEST.resolve():
            raise HarnessError(
                "formal measurement requires the canonical checked-in v2 manifest"
            )
        if v1.sha256_file(manifest_path) != EXPECTED_V2_MANIFEST_SHA:
            raise HarnessError("canonical v2 manifest digest changed")
    manifest = read_json_strict(manifest_path)
    suite_root = manifest_path.parent
    validate_manifest(manifest, suite_root)
    output_path = lexical_absolute_path(
        Path(arguments.output)
        if arguments.output
        else default_output_path(arguments.mode)
    )
    require_new_evidence_destination(
        output_path,
        arguments.mode,
        arguments.prepared_bundle,
    )
    repository = v1.repository_state(
        REPOSITORY_ROOT,
        require_clean=arguments.require_clean
        or arguments.mode in {"prepare", "measure"},
    )
    if arguments.mode in {"prepare", "measure"}:
        authority_origin = v1.git_capture(
            REPOSITORY_ROOT, ["remote", "get-url", "origin"]
        )
        if normalize_nomo_origin(authority_origin) != "github.com/nomo-lang/nomo":
            raise HarnessError(
                "benchmark authority checkout is not the official nomo-lang/nomo repository"
            )
        repository = {
            **repository,
            "origin_url": authority_origin,
            "normalized_origin": "github.com/nomo-lang/nomo",
            "manifest_sha256": EXPECTED_V2_MANIFEST_SHA,
        }
    toolchains = inspect_toolchains(
        manifest,
        arguments.nomo,
        arguments.clang,
        arguments.clangxx,
        arguments.go,
    )
    collector = select_collector()
    host = host_provenance()
    if arguments.mode == "correctness":
        result = run_correctness(
            arguments,
            manifest,
            manifest_path,
            suite_root,
            output_path,
            repository,
            toolchains,
            collector,
            host,
        )
    elif arguments.mode == "prepare":
        result = run_prepare(
            arguments,
            manifest,
            manifest_path,
            suite_root,
            output_path,
            repository,
            toolchains,
            collector,
            host,
        )
    else:
        result = run_measurement(
            arguments,
            manifest,
            manifest_path,
            suite_root,
            output_path,
            repository,
            toolchains,
            collector,
            host,
        )
    validate_result_schema(
        result, suite_root / "schema" / "result-v2.schema.json"
    )
    validate_result(result, manifest)
    validate_lexical_path_chain(
        output_path, "result output", final_kind="absent"
    )
    validate_lexical_path_chain(
        output_path.with_suffix(".log"),
        "result evidence log",
        final_kind="absent",
    )
    write_result_exclusive(output_path, result)
    write_evidence_log(output_path, result)
    return output_path, result


def write_evidence_log(output_path: Path, result: Dict[str, Any]) -> Path:
    log_path = output_path.with_suffix(".log")
    lines = [
        f"status={result.get('status')}",
        f"overall_verdict={result.get('overall_verdict')}",
        f"claim_eligible={result.get('claims', {}).get('claim_eligible')}",
    ]
    for failure in result.get("build_failures", []):
        lines.append("build_failure=")
        lines.append(json.dumps(failure, indent=2, sort_keys=True))
    for item in result.get("correctness", []):
        lines.append(
            "correctness="
            + json.dumps(
                {
                    "id": item.get("id"),
                    "status": item.get("status"),
                    "attempted_lanes": item.get("attempted_lanes"),
                    "failure_reason": item.get("failure_reason"),
                },
                sort_keys=True,
            )
        )
    try:
        with log_path.open("x", encoding="utf-8", newline="\n") as output:
            output.write("\n".join(lines) + "\n")
    except FileExistsError as error:
        raise HarnessError(
            f"evidence log appeared during the run; refusing to overwrite {log_path}"
        ) from error
    return log_path


def parse_arguments(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default=str(DEFAULT_MANIFEST))
    parser.add_argument(
        "--nomo", default=str(REPOSITORY_ROOT / "target" / "release" / "nomo")
    )
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--clangxx", default="clang++")
    parser.add_argument("--go", default="go")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument(
        "--mode",
        choices=("correctness", "prepare", "measure", "validate"),
        default="correctness",
    )
    parser.add_argument(
        "--artifact",
        help=(
            "result JSON or prepared bundle directory for offline, "
            "cross-host validation"
        ),
    )
    parser.add_argument("--candidate-checkout")
    parser.add_argument("--candidate-commit")
    parser.add_argument("--main-checkout")
    parser.add_argument("--main-commit")
    parser.add_argument("--environment-qualification")
    parser.add_argument("--prepared-bundle")
    parser.add_argument("--output")
    parser.add_argument("--require-clean", action="store_true")
    return parser.parse_args(argv)


def validate_artifact_offline(
    artifact_argument: str, manifest_argument: str
) -> Tuple[Path, Dict[str, Any]]:
    artifact = lexical_absolute_path(Path(artifact_argument))
    manifest_path = lexical_absolute_path(Path(manifest_argument))
    manifest = read_json_strict(manifest_path)
    validate_manifest(manifest, manifest_path.parent)
    result = (
        load_prepared_bundle(artifact)
        if artifact.is_dir()
        else read_json_strict(artifact)
    )
    validate_result_schema(
        result, manifest_path.parent / "schema" / "result-v2.schema.json"
    )
    validate_result(result, manifest, offline_replay=True)
    return artifact, result


def main(argv: Optional[Sequence[str]] = None) -> int:
    try:
        arguments = parse_arguments(argv)
        if arguments.mode == "validate":
            if not arguments.artifact:
                raise HarnessError(
                    "offline validate requires --artifact JSON-or-bundle"
                )
            artifact_path_value, result = validate_artifact_offline(
                arguments.artifact, arguments.manifest
            )
            print(f"validated {artifact_path_value}")
            print(f"status: {result['status']}")
            print(
                "formal parity: "
                f"{result['overall_verdict']}; "
                f"claim_eligible="
                f"{str(result['claims']['claim_eligible']).lower()}"
            )
            return 0
        output_path, result = run_suite(arguments)
    except HarnessError as error:
        print(f"benchmarksgame-v2: {error}", file=sys.stderr)
        return 1
    print(f"wrote {output_path}")
    print(f"evidence log: {output_path.with_suffix('.log')}")
    print(f"status: {result['status']}")
    for item in result["correctness"]:
        if item.get("status") == "completed":
            print(
                f"correctness {item['id']}: "
                f"{', '.join(item['attempted_lanes'])} match"
            )
        else:
            print(
                f"correctness {item['id']}: failed after "
                f"{', '.join(item['attempted_lanes'])}"
            )
    if result["status"] == "prepared":
        provenance = result["provenance"]
        print(f"prepared bundle: {provenance['prepared_bundle_path']}")
        print(
            "qualification request: "
            f"{provenance['qualification_request_path']}"
        )
        print(
            "prepared bundle sha256: "
            f"{provenance['prepared_bundle_sha256']}"
        )
        return 0
    if result["status"] in {"unavailable", "ineligible"}:
        print("formal parity: not evaluated; claim_eligible=false")
        return 2
    print(f"formal parity: {result['overall_verdict']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
