#!/usr/bin/env python3
"""Enforce canonical module roots and implicit-void source style."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


DECLARATION_VOID = re.compile(
    r"^\s*(?:pub\s+)?(?:suspend\s+)?fn\b.*\)\s*->\s*void\s*(?:\{|$)"
)
LEGACY_PACKAGE = re.compile(r"^\s*package\s+(?:app\.main|[A-Za-z_][A-Za-z0-9_]*\.main)\s*$")
RUST_LEGACY_PACKAGE = re.compile(
    r"\bpackage\s+(?:app\.main|[A-Za-z_][A-Za-z0-9_]*\.main)\b"
)
RUST_DECLARATION_VOID = re.compile(r"\)\s*->\s*void(?:\s*\{|\\n)")

# Compatibility syntax is executable policy, not a general fixture-writing shortcut.
# These exact counts cover the W0904 migration boundary and parser/formatter input.
RUST_LEGACY_ALLOWLIST = {
    Path("crates/nomo/src/project/module_root_migration.rs"): 6,
    Path("crates/nomo/tests/cli_project.rs"): 1,
}
RUST_VOID_ALLOWLIST = {
    Path("crates/nomo_fmt/src/lib.rs"): 4,
    Path("crates/nomo_syntax/src/parser_tests.rs"): 2,
}


def package_roots(repo: Path) -> list[Path]:
    roots: list[Path] = []
    for manifest in sorted(repo.rglob("nomo.toml")):
        if any(part in {".nomo", "build", "target", "vendor"} for part in manifest.parts):
            continue
        text = manifest.read_text(encoding="utf-8")
        if re.search(r"^\[package\]\s*$", text, re.MULTILINE) or re.search(
            r"^name\s*=", text, re.MULTILINE
        ):
            roots.append(manifest.parent)
    return roots


def source_violations(repo: Path) -> list[str]:
    violations: list[str] = []
    for path in sorted(repo.rglob("*.nomo")):
        if not path.is_file() or any(
            part in {".nomo", "build", "target", "vendor"} for part in path.parts
        ):
            continue
        relative = path.relative_to(repo)
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if DECLARATION_VOID.match(line):
                violations.append(
                    f"{relative}:{line_number}: declaration must omit `-> void`"
                )
            if LEGACY_PACKAGE.match(line):
                violations.append(
                    f"{relative}:{line_number}: legacy `.main` package declaration"
                )
    return violations


def embedded_fixture_violations(repo: Path) -> list[str]:
    violations: list[str] = []
    roots = ["crates", "libs", "std", "examples", "performance"]
    paths = sorted(
        path
        for root in roots
        for path in (repo / root).rglob("*.rs")
        if path.is_file()
        and not any(part in {"build", "target", "vendor"} for part in path.parts)
    )
    observed_legacy: dict[Path, int] = {}
    observed_void: dict[Path, int] = {}
    for path in paths:
        relative = path.relative_to(repo)
        text = path.read_text(encoding="utf-8")
        legacy_count = len(RUST_LEGACY_PACKAGE.findall(text))
        void_count = len(RUST_DECLARATION_VOID.findall(text))
        if legacy_count:
            observed_legacy[relative] = legacy_count
        if void_count:
            observed_void[relative] = void_count

    for relative in sorted(set(observed_legacy) | set(RUST_LEGACY_ALLOWLIST)):
        actual = observed_legacy.get(relative, 0)
        expected = RUST_LEGACY_ALLOWLIST.get(relative, 0)
        if actual != expected:
            violations.append(
                f"{relative}: expected {expected} intentional legacy package fixture(s), found {actual}"
            )
    for relative in sorted(set(observed_void) | set(RUST_VOID_ALLOWLIST)):
        actual = observed_void.get(relative, 0)
        expected = RUST_VOID_ALLOWLIST.get(relative, 0)
        if actual != expected:
            violations.append(
                f"{relative}: expected {expected} explicit-void compatibility fixture(s), found {actual}"
            )
    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nomo", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    repo = args.repo.resolve()
    binary = args.nomo.resolve()

    violations = source_violations(repo)
    violations.extend(embedded_fixture_violations(repo))
    for root in package_roots(repo):
        completed = subprocess.run(
            [str(binary), "fix", "module-roots", str(root), "--check"],
            cwd=repo,
            text=True,
            capture_output=True,
        )
        if completed.returncode:
            detail = (completed.stderr or completed.stdout).strip()
            violations.append(f"{root.relative_to(repo)}: {detail}")
    if violations:
        print("\n".join(violations))
        return 1
    print("canonical module roots and implicit-void declarations verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
