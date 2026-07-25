from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT / "scripts"))

import async_benchmark as benchmark  # noqa: E402


class AsyncBenchmarkTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = json.loads(
            (
                REPOSITORY_ROOT / "performance" / "async" / "manifest.json"
            ).read_text(encoding="utf-8")
        )

    def test_repository_manifest_is_valid_and_pins_go_patch(self) -> None:
        benchmark.validate_manifest(self.manifest)
        catalog = json.loads(
            (
                REPOSITORY_ROOT
                / "performance"
                / "async"
                / self.manifest["counter_catalog"]
            ).read_text(encoding="utf-8")
        )
        benchmark.validate_counter_catalog(catalog)
        self.assertEqual(
            self.manifest["toolchains"]["go"]["version"],
            "go1.25.12",
        )

    def test_manifest_rejects_fewer_than_five_runs(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["defaults"]["measured_runs"] = 4

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "at least five measured runs",
        ):
            benchmark.validate_manifest(manifest)

    def test_manifest_requires_every_rfc_workload(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["workloads"] = [
            workload
            for workload in manifest["workloads"]
            if workload["id"] != "cancellation_storm"
        ]

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "cancellation_storm",
        ):
            benchmark.validate_manifest(manifest)

    def test_counter_catalog_rejects_duplicate_names(self) -> None:
        catalog = {
            "schema": 1,
            "counters": [
                {
                    "name": "frame_allocations",
                    "unit": "count",
                    "available_phase": "P1",
                },
                {
                    "name": "frame_allocations",
                    "unit": "count",
                    "available_phase": "P1",
                },
            ],
        }

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "non-empty and unique",
        ):
            benchmark.validate_counter_catalog(catalog)

    def test_nearest_rank_and_summary_keep_tail_samples(self) -> None:
        samples = [
            {
                "wall_ns": value,
                "user_cpu_ns": value // 2,
                "system_cpu_ns": 0,
                "peak_rss_bytes": None,
            }
            for value in [10, 20, 30, 40, 50]
        ]

        summary = benchmark.summarize_samples(samples)

        self.assertEqual(summary["runs"], 5)
        self.assertEqual(summary["wall_median_ns"], 30)
        self.assertEqual(summary["wall_p50_ns"], 30)
        self.assertEqual(summary["wall_p99_ns"], 50)
        self.assertEqual(summary["wall_p999_ns"], 50)
        self.assertIsNone(summary["peak_rss_bytes"])

    def test_static_gate_reports_forbidden_generated_c_symbol(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "main.c"
            source.write_text("void nomo_executor_start(void) {}\n", encoding="utf-8")
            snapshot = {
                "gate": "async-unused",
                "required_absent_generated_c_patterns": ["nomo_executor"],
            }

            with self.assertRaisesRegex(
                benchmark.HarnessError,
                "nomo_executor=1",
            ):
                benchmark.scan_static_gate(source, snapshot)

    def test_static_gate_records_zero_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "main.c"
            source.write_text("int main(void) { return 0; }\n", encoding="utf-8")
            snapshot = {
                "gate": "async-unused",
                "required_absent_generated_c_patterns": [
                    "nomo_executor",
                    "__atomic_",
                ],
            }

            result = benchmark.scan_static_gate(source, snapshot)

        self.assertTrue(result["passed"])
        self.assertEqual(
            result["forbidden_pattern_counts"],
            {"nomo_executor": 0, "__atomic_": 0},
        )

    def test_p0_result_rejects_a_performance_claim(self) -> None:
        result = {
            "schema": 1,
            "suite": "nomo-async-runtime",
            "phase": "P0",
            "claims": {
                "performance_claim_allowed": True,
            },
            "workloads": [],
        }

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "must not allow a performance claim",
        ):
            benchmark.validate_result(result, 5)


if __name__ == "__main__":
    unittest.main()
