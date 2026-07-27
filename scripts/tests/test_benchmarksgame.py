from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT / "scripts"))

import benchmarksgame as benchmark  # noqa: E402


class BenchmarksGameTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest_path = (
            REPOSITORY_ROOT / "performance" / "benchmarksgame" / "manifest.json"
        )
        self.manifest = json.loads(
            self.manifest_path.read_text(encoding="utf-8")
        )

    def test_repository_manifest_and_schema_are_valid(self) -> None:
        benchmark.validate_manifest(
            self.manifest,
            self.manifest_path.parent,
        )
        schema = json.loads(
            (
                self.manifest_path.parent / "schema" / "result.schema.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(schema["$defs"]["implementation"]["properties"]["samples"]["maxItems"], 12)
        self.assertEqual(
            [item["id"] for item in self.manifest["readiness"]],
            list(benchmark.READINESS_IDS),
        )

    def test_manifest_rejects_changed_source_content(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["workloads"][0]["sources"]["c"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(benchmark.HarnessError, "SHA-256 mismatch"):
            benchmark.validate_manifest(manifest, self.manifest_path.parent)

    def test_toolchain_mismatch_is_reported(self) -> None:
        with mock.patch.object(
            benchmark.shutil,
            "which",
            return_value=sys.executable,
        ), mock.patch.object(
            benchmark,
            "tool_version",
            side_effect=[
                "nomo 0.0.0-20260721120555\n\nCommands:",
                "Apple clang version 21.0.0",
                "go version go1.25.11 darwin/arm64",
            ],
        ):
            with self.assertRaisesRegex(
                benchmark.ToolchainMismatch,
                "Go expected go1.25.12, found go1.25.11",
            ):
                benchmark.inspect_toolchains(
                    self.manifest,
                    Path(sys.executable),
                    "clang",
                    "go",
                )

    def test_output_mismatch_is_rejected(self) -> None:
        with self.assertRaisesRegex(benchmark.HarnessError, "output mismatch"):
            benchmark.timed_run(
                [sys.executable, "-c", "print('wrong')"],
                expected_stdout=b"right\n",
                timeout_seconds=5.0,
            )

    def test_dirty_checkout_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(
                ["git", "init", "-q"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Benchmark Test"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.email", "benchmark@example.invalid"],
                cwd=repository,
                check=True,
            )
            tracked = repository / "tracked.txt"
            tracked.write_text("clean\n", encoding="utf-8")
            subprocess.run(["git", "add", "tracked.txt"], cwd=repository, check=True)
            subprocess.run(
                ["git", "commit", "-q", "-m", "fixture"],
                cwd=repository,
                check=True,
            )
            state = benchmark.repository_state(repository, require_clean=True)
            self.assertFalse(state["dirty"])
            tracked.write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(
                benchmark.HarnessError,
                "dirty checkout is not allowed",
            ):
                benchmark.repository_state(repository, require_clean=True)

    def test_hard_timeout_kills_the_child(self) -> None:
        with self.assertRaisesRegex(
            benchmark.CommandTimedOut,
            "was killed",
        ):
            benchmark.timed_run(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                expected_stdout=b"",
                timeout_seconds=0.02,
            )

    def test_twelve_run_statistics_follow_the_contract(self) -> None:
        samples = [
            {
                "wall_ns": value * 1_000_000,
                "cpu_total_ns": value * 2_000_000,
                "peak_rss_bytes": value * 1024,
            }
            for value in range(1, 13)
        ]
        summary = benchmark.summarize_samples(samples)
        self.assertEqual(summary["measurement_mode"], "twelve-run")
        self.assertEqual(summary["runs"], 12)
        self.assertEqual(summary["wall_min_ns"], 1_000_000)
        self.assertEqual(summary["wall_median_ns"], 6_500_000)
        self.assertEqual(summary["cpu_sample_count"], 11)
        self.assertEqual(summary["cpu_excluded_run_indices"], [1])
        self.assertEqual(summary["peak_rss_max_bytes"], 12 * 1024)

    def test_confidence_interval_uses_student_t(self) -> None:
        interval = benchmark.confidence_interval_95(
            [value * 1_000_000 for value in range(1, 12)]
        )
        self.assertEqual(interval["sample_count"], 11)
        self.assertEqual(interval["degrees_of_freedom"], 10)
        self.assertEqual(interval["mean_ns"], 6_000_000)
        self.assertAlmostEqual(interval["half_width_ns"], 2_228_138.85196, places=4)
        self.assertAlmostEqual(interval["lower_ns"], 3_771_861.14804, places=4)
        self.assertAlmostEqual(interval["upper_ns"], 8_228_138.85196, places=4)

    def test_single_run_summary_does_not_masquerade_as_twelve(self) -> None:
        summary = benchmark.summarize_samples(
            [
                {
                    "wall_ns": 601_000_000_000,
                    "cpu_total_ns": 600_000_000_000,
                    "peak_rss_bytes": 4096,
                }
            ]
        )
        self.assertEqual(summary["measurement_mode"], "single-run-over-slow-cutoff")
        self.assertEqual(summary["runs"], 1)
        self.assertIsNone(summary["wall_iqr_ns"])
        self.assertIsNone(summary["cpu_mean_ns"])
        self.assertIsNone(summary["cpu_ci_95"])

    def test_result_requires_complete_top_level_fields(self) -> None:
        result = self.valid_correctness_result()
        benchmark.validate_result(result)
        del result["provenance"]
        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "missing fields: provenance",
        ):
            benchmark.validate_result(result)

    def test_complete_compile_commands_are_preserved(self) -> None:
        builds = self.valid_builds()
        benchmark.validate_compile_command_provenance(builds)
        record = builds["spectral-norm"]["commands"]["nomo_clang"]
        record["argv"].remove("-O3")
        record["command"] = benchmark.command_text(record["argv"])
        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "lost the fixed Clang flags",
        ):
            benchmark.validate_compile_command_provenance(builds)

    def valid_builds(self) -> dict:
        builds = {}
        for workload in benchmark.WORKLOAD_IDS:
            clang_argv = [
                "/usr/bin/clang",
                *benchmark.BASE_CLANG_FLAGS,
                "main.c",
                "-o",
                "program",
            ]
            builds[workload] = {
                "commands": {
                    "nomo_emit_c": self.command_record(
                        ["/tmp/nomo", "build", "project", "--emit-c"]
                    ),
                    "nomo_clang": self.command_record(clang_argv),
                    "c_clang": self.command_record(clang_argv),
                    "go_build": self.command_record(
                        ["/usr/bin/go", "build", "-o", "program", "main.go"]
                    ),
                }
            }
        return builds

    def command_record(self, argv: list[str]) -> dict:
        copied = list(argv)
        return {
            "argv": copied,
            "command": benchmark.command_text(copied),
        }

    def valid_correctness_result(self) -> dict:
        return {
            "schema": 1,
            "suite": "nomo-benchmarksgame-cpu-baseline",
            "manifest_version": "2026-07-27",
            "mode": "correctness",
            "created_at_utc": "2026-07-27T00:00:00+00:00",
            "claims": {
                "exploratory": True,
                "affinity_enforced": False,
                "claim_eligible": False,
                "scope": (
                    "single-thread CPU, Array/COW behavior, floating point, "
                    "and C99 code generation only"
                ),
            },
            "provenance": {},
            "builds": self.valid_builds(),
            "correctness": [{}, {}, {}],
            "workloads": [],
        }


if __name__ == "__main__":
    unittest.main()
