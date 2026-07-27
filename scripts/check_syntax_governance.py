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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nomo", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    repo = args.repo.resolve()
    binary = args.nomo.resolve()

    violations = source_violations(repo)
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
