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
        self.p1_manifest = json.loads(
            (
                REPOSITORY_ROOT / "performance" / "async" / "manifest-p1.json"
            ).read_text(encoding="utf-8")
        )
        self.p3_manifest = json.loads(
            (
                REPOSITORY_ROOT / "performance" / "async" / "manifest-p3.json"
            ).read_text(encoding="utf-8")
        )
        self.catalog = json.loads(
            (
                REPOSITORY_ROOT
                / "performance"
                / "async"
                / self.manifest["counter_catalog"]
            ).read_text(encoding="utf-8")
        )

    def test_repository_manifest_is_valid_and_pins_go_patch(self) -> None:
        benchmark.validate_manifest(self.manifest)
        benchmark.validate_counter_catalog(self.catalog)
        benchmark.validate_manifest(self.p1_manifest)
        benchmark.validate_manifest(self.p3_manifest)
        self.assertEqual(self.p1_manifest["phase"], "P1")
        self.assertEqual(self.p3_manifest["phase"], "P3")
        bounded_channel = next(
            workload
            for workload in self.p3_manifest["workloads"]
            if workload["id"] == "bounded_channel"
        )
        self.assertTrue(bounded_channel["enabled"])
        self.assertIn("go_project", bounded_channel)
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

    def test_p1_manifest_requires_an_enabled_counter_gate(self) -> None:
        manifest = copy.deepcopy(self.p1_manifest)
        for workload in manifest["workloads"]:
            if workload["kind"] == "runtime_counter_gate":
                workload["enabled"] = False

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "must enable a runtime counter gate",
        ):
            benchmark.validate_manifest(manifest)

    def test_manifest_rejects_invalid_expected_exit_code(self) -> None:
        manifest = copy.deepcopy(self.p1_manifest)
        manifest["workloads"][0]["expected_exit_code"] = -1

        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "invalid expected exit code",
        ):
            benchmark.validate_manifest(manifest)

    def test_timed_run_accepts_an_expected_failure_contract(self) -> None:
        sample, stdout = benchmark.timed_run(
            [
                sys.executable,
                "-c",
                "import sys; print('out'); print('err', file=sys.stderr); raise SystemExit(7)",
            ],
            b"out\n",
            b"err\n",
            7,
            5.0,
        )

        self.assertEqual(stdout, b"out\n")
        self.assertGreater(sample["wall_ns"], 0)

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
        self.assertEqual(result["required_pattern_counts"], {})

    def test_static_gate_requires_exact_generated_c_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "main.c"
            source.write_text(
                "void nomo_async_metrics_export(void) {}\n"
                "nomo_async_metrics_export();\n",
                encoding="utf-8",
            )
            snapshot = {
                "gate": "p1-runtime-counters",
                "required_absent_generated_c_patterns": [],
                "required_generated_c_pattern_counts": {
                    "nomo_async_metrics_export": 2,
                },
            }

            result = benchmark.scan_static_gate(source, snapshot)
            self.assertEqual(
                result["required_pattern_counts"],
                {"nomo_async_metrics_export": 2},
            )
            snapshot["required_generated_c_pattern_counts"][
                "nomo_async_metrics_export"
            ] = 1
            with self.assertRaisesRegex(
                benchmark.HarnessError,
                "expected:1,found:2",
            ):
                benchmark.scan_static_gate(source, snapshot)

    def test_runtime_counter_payload_is_catalog_checked_and_exact(self) -> None:
        payload = {
            "schema": 1,
            "runtime": "nomo-c99-current-thread",
            "runtime_abi": 1,
            "counter_catalog_schema": 1,
            "counters": {
                "poll_calls": 5,
                "cooperative_yields": 2,
                "frame_allocations": 0,
                "frame_drops": 2,
                "peak_live_frames": 2,
                "ready_queue_enqueues": 2,
                "ready_queue_dequeues": 2,
                "ready_queue_saturations": 0,
                "ready_queue_cancellations": 0,
                "task_spawns": 0,
                "task_joins": 0,
                "join_suspensions": 0,
                "task_cancellations": 0,
                "deadline_registrations": 0,
                "deadline_expirations": 0,
                "deadline_cancellations": 0,
                "timer_registrations": 0,
                "timer_expirations": 0,
                "timer_cancellations": 0,
                "live_timers": 0,
                "peak_live_timers": 0,
            },
            "unavailable": {
                "local_retain": "not implemented",
                "local_release": "not implemented",
            },
        }
        expected = {
            "counters": {
                "poll_calls": 5,
                "frame_allocations": 0,
            },
            "unavailable": [
                "local_retain",
                "local_release",
            ],
        }

        benchmark.validate_runtime_counter_payload(
            payload,
            self.catalog,
            expected,
        )

        invalid = copy.deepcopy(payload)
        invalid["counters"]["poll_calls"] = -1
        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "non-negative integer",
        ):
            benchmark.validate_runtime_counter_payload(
                invalid,
                self.catalog,
                expected,
            )

        unknown = copy.deepcopy(payload)
        unknown["counters"]["mystery"] = 1
        with self.assertRaisesRegex(
            benchmark.HarnessError,
            "unknown counters: mystery",
        ):
            benchmark.validate_runtime_counter_payload(
                unknown,
                self.catalog,
                expected,
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
            "pre-reactor result must not allow a performance claim",
        ):
            benchmark.validate_result(result, 5)


if __name__ == "__main__":
    unittest.main()
