from __future__ import annotations

import copy
import contextlib
import ctypes
import datetime as dt
import base64
import json
import math
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from argparse import Namespace
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT / "scripts"))

import benchmarksgame as v1  # noqa: E402
import benchmarksgame_v2 as benchmark  # noqa: E402


class FakeFunction:
    argtypes = None
    restype = None


class FakeLibrary:
    def __init__(self, names: tuple[str, ...]) -> None:
        for name in names:
            setattr(self, name, FakeFunction())


class BenchmarksGameV2Tests(unittest.TestCase):
    _rust_toolchain_fixture: dict | None = None

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.manifest_path = (
            REPOSITORY_ROOT
            / "performance"
            / "benchmarksgame"
            / "manifest-v2.json"
        )
        self.suite_root = self.manifest_path.parent
        self.manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        self.result_schema_path = (
            self.suite_root / "schema" / "result-v2.schema.json"
        )

    def test_repository_manifest_schema_and_v1_predecessor_are_valid(self) -> None:
        benchmark.validate_manifest(self.manifest, self.suite_root)
        schema = json.loads(self.result_schema_path.read_text(encoding="utf-8"))
        self.assertEqual(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
        )
        self.assertEqual(schema["properties"]["schema"]["const"], 2)
        self.assertEqual(
            schema["$defs"]["laneSamples30"]["properties"]["c"]["minItems"], 30
        )
        self.assertIn("origin_main_commit", schema["$defs"]["compilerBuild"]["properties"])
        self.assertNotIn("origin_main_commit", schema["$defs"]["releaseLane"]["properties"])
        self.assertIn("cargo_configs", schema["$defs"]["compilerBuild"]["required"])
        self.assertEqual(
            set(
                schema["$defs"]["compilerBuild"]["properties"][
                    "environment"
                ]["required"]
            ),
            {"CARGO_TARGET_DIR", "CARGO_HOME", "RUSTC"},
        )
        self.assertIn("rustc", schema["$defs"]["compilerBuild"]["required"])
        self.assertIn(
            "build_environment",
            schema["$defs"]["provenance"]["properties"]["toolchains"][
                "required"
            ],
        )
        self.assertIn(
            "runtime_environments",
            schema["$defs"]["provenance"]["properties"]["toolchains"][
                "required"
            ],
        )
        self.assertTrue(
            {
                "build_metadata_path",
                "build_metadata_sha256",
                "build_metadata",
            }.issubset(schema["$defs"]["releaseBuild"]["required"])
        )
        self.assertFalse(
            schema["$defs"]["buildMetadata"]["additionalProperties"]
        )
        self.assertIn("stability", schema["$defs"]["batch"]["required"])
        self.assertIn("evaluation", schema["$defs"]["batch"]["required"])
        predecessor = self.manifest["predecessor"]
        self.assertEqual(
            predecessor["manifest_sha256"], benchmark.EXPECTED_V1_MANIFEST_SHA
        )
        self.assertEqual(
            v1.sha256_file(self.suite_root / predecessor["manifest_path"]),
            benchmark.EXPECTED_V1_MANIFEST_SHA,
        )
        v1_manifest = json.loads(
            (self.suite_root / predecessor["manifest_path"]).read_text(
                encoding="utf-8"
            )
        )
        v1.validate_manifest(v1_manifest, self.suite_root)

    def test_frozen_benchmark_files_are_git_lf_and_byte_stable(self) -> None:
        paths = [
            "performance/benchmarksgame/manifest-v2.json",
            "performance/benchmarksgame/reference/c/spectral-norm.c",
            "performance/benchmarksgame/reference/cpp/spectral-norm.cpp",
            "performance/benchmarksgame/reference/go/spectral-norm.go",
            "performance/benchmarksgame/reference/nomo/spectral-norm/src/main.nomo",
            "performance/benchmarksgame/reference/nomo/spectral-norm/nomo.toml",
            "performance/benchmarksgame/fixtures/spectral-norm-100.txt",
            "performance/benchmarksgame/README-v2.md",
            "scripts/benchmarksgame.py",
            "scripts/benchmarksgame_v2.py",
        ]
        for relative in paths:
            with self.subTest(path=relative):
                attribute = subprocess.run(
                    ["git", "check-attr", "eol", "--", relative],
                    cwd=REPOSITORY_ROOT,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()
                self.assertTrue(attribute.endswith("eol: lf"), attribute)
                content = (REPOSITORY_ROOT / relative).read_bytes()
                self.assertNotIn(b"\r\n", content)
        benchmark.validate_manifest(self.manifest, self.suite_root)

    def test_manifest_pins_rfc_amendment_two_modes_and_decisive_drift(self) -> None:
        self.assertEqual(
            self.manifest["rfc"]["allocation_clarification_merge_commit"],
            "75a7e14adc1ea06ccdc9a28c1dc0676ce8404a1c",
        )
        self.assertEqual(
            self.manifest["methodology"]["formal_build_modes"],
            ["release", "emit-c"],
        )
        invalidation = self.manifest["methodology"]["batch_invalidation"]
        self.assertEqual(invalidation["decisive_reference_lanes"], ["c", "cpp"])
        self.assertEqual(invalidation["diagnostic_reference_lanes"], ["go"])
        self.assertEqual(
            self.manifest["toolchains"]["clang"]["driver_config_flags"],
            list(benchmark.CLANG_DRIVER_CONFIG_FLAGS),
        )
        self.assertEqual(
            self.manifest["toolchains"]["go"]["build_environment"],
            benchmark.GO_BUILD_ENVIRONMENT_CONTRACT,
        )

    def test_manifest_rejects_frozen_input_threshold_or_rfc_changes(self) -> None:
        changed_input = copy.deepcopy(self.manifest)
        changed_input["workloads"][0]["performance_input"] = "5499"
        with self.assertRaisesRegex(benchmark.HarnessError, "formal input changed"):
            benchmark.validate_manifest(changed_input, self.suite_root)
        changed_threshold = copy.deepcopy(self.manifest)
        changed_threshold["thresholds"]["workload"]["c_u99_max"] = 1.06
        with self.assertRaisesRegex(benchmark.HarnessError, "thresholds changed"):
            benchmark.validate_manifest(changed_threshold, self.suite_root)
        changed_rfc = copy.deepcopy(self.manifest)
        changed_rfc["rfc"]["allocation_clarification_merge_commit"] = "0" * 40
        with self.assertRaisesRegex(benchmark.HarnessError, "allocation amendment"):
            benchmark.validate_manifest(changed_rfc, self.suite_root)
        changed_cpp = copy.deepcopy(self.manifest)
        changed_cpp["workloads"][0]["sources"]["cpp"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(benchmark.HarnessError, "source SHA set changed"):
            benchmark.validate_manifest(changed_cpp, self.suite_root)
        duplicate_checks = copy.deepcopy(self.manifest)
        duplicate_checks["environment_qualification"]["required_checks"] = [
            "canonical_host_identity"
        ] * 14
        with self.assertRaisesRegex(benchmark.HarnessError, "exact unique ordered"):
            benchmark.validate_manifest(duplicate_checks, self.suite_root)

    def test_cpp_references_are_strict_iso_cpp20_and_allocation_equivalent(self) -> None:
        clangxx = shutil.which("clang++")
        if clangxx is None:
            self.skipTest("clang++ is unavailable")
        for workload in benchmark.WORKLOAD_IDS:
            source = self.suite_root / "reference" / "cpp" / f"{workload}.cpp"
            subprocess.run(
                [
                    clangxx,
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    "-std=c++20",
                    "-pedantic-errors",
                    "-fsyntax-only",
                    str(source),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            manifest_source = next(
                item for item in self.manifest["workloads"] if item["id"] == workload
            )["sources"]["cpp"]
            self.assertEqual(v1.sha256_file(source), manifest_source["sha256"])
        spectral = (
            self.suite_root / "reference" / "cpp" / "spectral-norm.cpp"
        ).read_text(encoding="utf-8")
        fannkuch = (
            self.suite_root / "reference" / "cpp" / "fannkuch-redux.cpp"
        ).read_text(encoding="utf-8")
        for text in (spectral, fannkuch):
            self.assertIn("std::unique_ptr", text)
            self.assertNotIn("std::vector", text)
            self.assertNotIn("push_back", text)
            self.assertNotIn("reserve(", text)
            self.assertNotIn("resize(", text)
            self.assertNotIn("<thread>", text)
        self.assertEqual(spectral.count("new double[N]"), 3)
        self.assertEqual(fannkuch.count("new int[n]"), 3)
        self.assertIn("-pedantic-errors", benchmark.BASE_CPP_FLAGS)
        for workload in ("spectral-norm", "fannkuch-redux"):
            source = next(
                item for item in self.manifest["workloads"] if item["id"] == workload
            )["sources"]["cpp"]
            self.assertIn("stack-to-dynamic", source["derivation"])
            self.assertIn("without growth or reallocation", source["derivation"])

    def test_frozen_source_lock_includes_rfc_allocation_derivation(self) -> None:
        source_lock = benchmark.frozen_source_lock(self.manifest)
        self.assertEqual(len(source_lock), 3)
        for item in source_lock:
            self.assertEqual(
                item["allocation_contract_rfc_commit"],
                "75a7e14adc1ea06ccdc9a28c1dc0676ce8404a1c",
            )
            self.assertIn("line-and-algorithm-equivalent", item["cpp_derivation"])
            mappings = item["cpp_allocation_mappings"]
            self.assertEqual(
                len(mappings),
                1 if item["id"] == "n-body" else 3,
            )
            self.assertTrue(
                all(
                    mapping["no_growth_or_reallocation"]
                    and not mapping["custom_allocator"]
                    for mapping in mappings
                )
            )
            cpp_source = next(
                workload
                for workload in self.manifest["workloads"]
                if workload["id"] == item["id"]
            )["sources"]["cpp"]
            text = (
                self.suite_root / cpp_source["path"]
            ).read_text(encoding="utf-8")
            for mapping in mappings:
                representation = mapping["iso_cpp20_representation"]
                self.assertIn(
                    representation.split("(", 1)[0].strip(),
                    text,
                )

    def test_williams_schedule_is_position_and_carryover_balanced(self) -> None:
        schedule = benchmark.williams_schedule()
        benchmark.validate_williams_schedule(schedule)
        self.assertEqual(len(schedule), 30)
        for lane in benchmark.TIMED_LANES:
            self.assertEqual(
                [
                    sum(row[position] == lane for row in schedule)
                    for position in range(5)
                ],
                [6, 6, 6, 6, 6],
            )
        for left in benchmark.TIMED_LANES:
            for right in benchmark.TIMED_LANES:
                if left == right:
                    continue
                count = sum(
                    pair == (left, right)
                    for row in schedule
                    for pair in zip(row, row[1:])
                )
                self.assertEqual(count, 6)

    def test_williams_schedule_rejects_tampering(self) -> None:
        schedule = benchmark.williams_schedule()
        schedule[0] = list(schedule[1])
        with self.assertRaisesRegex(benchmark.HarnessError, "not position-balanced"):
            benchmark.validate_williams_schedule(schedule)

    def test_one_sided_99_percent_log_ratio_matches_known_vector(self) -> None:
        result = benchmark.paired_log_statistics(
            [value * 1_000_000 for value in range(101, 131)],
            [100_000_000] * 30,
        )
        self.assertEqual(result["sample_count"], 30)
        self.assertEqual(result["degrees_of_freedom"], 29)
        self.assertEqual(result["t_critical"], benchmark.T_CRITICAL_99_DF29)
        self.assertAlmostEqual(result["mean_log_ratio"], 0.14127814022762147)
        self.assertAlmostEqual(result["sample_standard_deviation"], 0.07652150006275839)
        self.assertAlmostEqual(result["standard_error"], 0.013970850572835251)
        self.assertAlmostEqual(result["point_ratio"], 1.1517449500408021)
        self.assertAlmostEqual(result["upper_bound_99"], 1.1920501891607673)

    def test_suite_statistics_use_equal_workload_weights(self) -> None:
        result = benchmark.suite_log_statistics(
            [[math.log(2.0)] * 30, [0.0] * 30, [0.0] * 30]
        )
        self.assertAlmostEqual(result["point_ratio"], 2.0 ** (1.0 / 3.0))
        self.assertAlmostEqual(result["upper_bound_99"], 2.0 ** (1.0 / 3.0))

    def test_per_workload_and_suite_thresholds_are_all_decisive(self) -> None:
        passing = [
            self.synthetic_workload(workload, 100, 100)
            for workload in benchmark.WORKLOAD_IDS
        ]
        result = benchmark.evaluate_batch(passing, self.manifest["thresholds"], True)
        self.assertEqual(result["verdict"], "pass")
        failing = copy.deepcopy(passing)
        failing[1]["samples"]["cpp"] = [{"wall_ns": 90}] * 30
        result = benchmark.evaluate_batch(failing, self.manifest["thresholds"], True)
        self.assertEqual(result["verdict"], "fail")
        self.assertEqual(result["workloads"][1]["verdict"], "fail")
        ineligible = benchmark.evaluate_batch(
            passing, self.manifest["thresholds"], False
        )
        self.assertEqual(ineligible["verdict"], "ineligible")

    def test_go_drift_is_diagnostic_and_does_not_invalidate_batch(self) -> None:
        workloads = self.stability_workloads(
            candidate=[100.0] * 30,
            c=[100.0] * 30,
            cpp=[100.0] * 30,
            main=[100.0] * 30,
            go=[100.0] * 15 + [200.0] * 15,
        )
        result = benchmark.batch_stability(
            workloads, self.manifest["methodology"]["batch_invalidation"]
        )
        self.assertTrue(result["valid"])
        self.assertEqual(result["issues"], [])
        self.assertEqual(len(result["warnings"]), 3)
        self.assertTrue(
            all(not item["passed"] for item in result["diagnostic_reference_drift"])
        )

    def test_reference_drift_two_percent_boundary_is_inclusive(self) -> None:
        workloads = self.stability_workloads(
            candidate=[100.0] * 30,
            c=[100.0] * 15 + [102.0] * 15,
            cpp=[100.0] * 30,
            main=[100.0] * 30,
            go=[100.0] * 30,
        )
        result = benchmark.batch_stability(
            workloads, self.manifest["methodology"]["batch_invalidation"]
        )
        self.assertTrue(result["valid"])
        workloads[0]["samples"]["c"][29]["wall_ns"] = 102.1
        result = benchmark.batch_stability(
            workloads, self.manifest["methodology"]["batch_invalidation"]
        )
        self.assertFalse(result["valid"])

    def test_paired_ratio_rsd_three_percent_boundary_is_inclusive(self) -> None:
        delta = 0.03 * math.sqrt(29.0 / 30.0)
        candidate = [1.0 - delta] * 15 + [1.0 + delta] * 15
        workloads = self.stability_workloads(
            candidate=candidate,
            c=[1.0] * 30,
            cpp=[1.0] * 30,
            main=[1.0] * 30,
            go=[1.0] * 30,
        )
        result = benchmark.batch_stability(
            workloads, self.manifest["methodology"]["batch_invalidation"]
        )
        self.assertTrue(result["valid"])
        over_delta = 0.03001 * math.sqrt(29.0 / 30.0)
        for workload in workloads:
            workload["samples"]["candidate"] = [
                {"wall_ns": value}
                for value in [1.0 - over_delta] * 15 + [1.0 + over_delta] * 15
            ]
        result = benchmark.batch_stability(
            workloads, self.manifest["methodology"]["batch_invalidation"]
        )
        self.assertFalse(result["valid"])

    def test_retry_retains_invalid_artifact_and_never_pools_batches(self) -> None:
        invalid = self.stability_summary(False, ["reference drift"])
        valid = self.stability_summary(True, [])
        with mock.patch.object(
            benchmark, "batch_stability", side_effect=[invalid, valid, valid]
        ), mock.patch.object(
            benchmark,
            "evaluate_batch",
            return_value={"environment_eligible": True, "verdict": "pass"},
        ):
            batches = benchmark.collect_protocol_batches(
                self.manifest,
                self.suite_root,
                {workload: {} for workload in benchmark.WORKLOAD_IDS},
                mock.Mock(),
                "release",
                self.static_authorization_stub(),
                measure_function=self.stub_measurement,
                dynamic_snapshot_function=self.dynamic_snapshot_factory(),
            )
        self.assertEqual(
            [batch["status"] for batch in batches],
            ["invalidated", "completed", "completed"],
        )
        self.assertEqual(
            [batch["batch_index"] for batch in batches], [1, 1, 2]
        )
        self.assertEqual(
            [batch["attempt_index"] for batch in batches], [1, 2, 3]
        )
        self.assertEqual(batches[0]["invalidation_reasons"], ["reference drift"])
        self.assertIsNot(batches[1]["workloads"], batches[2]["workloads"])

    def test_second_anomaly_terminates_after_one_automatic_rerun(self) -> None:
        invalid = self.stability_summary(False, ["reference drift"])
        with mock.patch.object(
            benchmark, "batch_stability", side_effect=[invalid, invalid]
        ):
            batches = benchmark.collect_protocol_batches(
                self.manifest,
                self.suite_root,
                {workload: {} for workload in benchmark.WORKLOAD_IDS},
                mock.Mock(),
                "emit-c",
                self.static_authorization_stub(),
                measure_function=self.stub_measurement,
                dynamic_snapshot_function=self.dynamic_snapshot_factory(),
            )
        self.assertEqual(len(batches), 2)
        self.assertEqual(
            [batch["status"] for batch in batches],
            ["invalidated", "invalidated"],
        )
        self.assertEqual([batch["batch_index"] for batch in batches], [1, 1])

    def test_two_independent_passing_batches_are_required(self) -> None:
        valid = self.stability_summary(True, [])
        with mock.patch.object(
            benchmark, "batch_stability", side_effect=[valid, valid]
        ), mock.patch.object(
            benchmark,
            "evaluate_batch",
            return_value={"environment_eligible": True, "verdict": "pass"},
        ):
            batches = benchmark.collect_protocol_batches(
                self.manifest,
                self.suite_root,
                {workload: {} for workload in benchmark.WORKLOAD_IDS},
                mock.Mock(),
                "release",
                self.static_authorization_stub(),
                measure_function=self.stub_measurement,
                dynamic_snapshot_function=self.dynamic_snapshot_factory(),
            )
        self.assertEqual(len(batches), 2)
        self.assertEqual([batch["batch_index"] for batch in batches], [1, 2])
        self.assertEqual(
            sum(len(batch["workloads"]) for batch in batches), 6
        )

    def test_partial_failure_retains_incremental_raw_evidence(self) -> None:
        def fail_after_one(
            _manifest,
            _suite_root,
            workload,
            _binaries,
            _collector,
            build_mode,
            batch_index,
            attempt_index,
            evidence,
        ):
            evidence.update(
                {
                    "id": workload["id"],
                    "build_mode": build_mode,
                    "collection_status": "failed",
                    "failure_reason": "fixture timeout",
                    "warmups": {
                        lane: ([{"wall_ns": 1}] if lane == "candidate" else [])
                        for lane in benchmark.TIMED_LANES
                    },
                    "samples": {lane: [] for lane in benchmark.TIMED_LANES},
                    "block_schedule": [],
                }
            )
            raise benchmark.HarnessError("fixture timeout")

        batches = benchmark.collect_protocol_batches(
            self.manifest,
            self.suite_root,
            {workload: {} for workload in benchmark.WORKLOAD_IDS},
            mock.Mock(),
            "release",
            self.static_authorization_stub(),
            measure_function=fail_after_one,
            dynamic_snapshot_function=self.dynamic_snapshot_factory(),
        )
        self.assertEqual(len(batches), 2)
        for batch in batches:
            self.assertEqual(batch["status"], "invalidated")
            self.assertEqual(batch["workloads"][0]["failure_reason"], "fixture timeout")
            self.assertEqual(
                batch["workloads"][0]["warmups"]["candidate"], [{"wall_ns": 1}]
            )

    def test_after_snapshot_failure_keeps_complete_samples_and_stability(self) -> None:
        result = self.completed_result()
        authorization = result["provenance"]["environment_qualification"]
        base_factory = self.dynamic_snapshot_factory()
        call_index = 0

        def snapshots(authority_host_sha256: str, policy=None) -> dict:
            nonlocal call_index
            call_index += 1
            snapshot = base_factory(authority_host_sha256, policy)
            if call_index == 2:
                thermal = snapshot["observations"]["thermal_state"]
                thermal["status"] = "failed"
                thermal["parsed"]["normal"] = False
                snapshot["eligible"] = False
                snapshot["reason"] = "thermal anomaly"
                body = {
                    key: value
                    for key, value in snapshot.items()
                    if key != "snapshot_sha256"
                }
                snapshot["snapshot_sha256"] = benchmark.canonical_json_sha256(body)
            return snapshot

        def measured(
            _manifest,
            _suite_root,
            workload,
            _binaries,
            _collector,
            build_mode,
            batch_index,
            attempt_index,
            evidence,
        ):
            evidence.update(
                self.measured_workload(
                    workload["id"], build_mode, batch_index, attempt_index
                )
            )
            return evidence

        batches = benchmark.collect_protocol_batches(
            self.manifest,
            self.suite_root,
            {workload: {} for workload in benchmark.WORKLOAD_IDS},
            mock.Mock(),
            "release",
            authorization,
            measure_function=measured,
            dynamic_snapshot_function=snapshots,
        )
        self.assertEqual(
            [batch["status"] for batch in batches],
            ["invalidated", "completed", "completed"],
        )
        self.assertIsNotNone(batches[0]["stability"])
        self.assertEqual(len(batches[0]["workloads"]), 3)
        self.assertTrue(
            all(
                workload["collection_status"] == "completed"
                for workload in batches[0]["workloads"]
            )
        )

    def test_protocol_result_wires_correctness_then_authorized_batches(self) -> None:
        authorization = self.static_authorization_stub()
        correctness = [
            {"id": workload, "status": "completed"}
            for workload in benchmark.WORKLOAD_IDS
        ]
        batches = [
            {
                "status": "completed",
                "evaluation": {"verdict": "pass"},
            },
            {
                "status": "completed",
                "evaluation": {"verdict": "pass"},
            },
        ]
        for build_mode in benchmark.FORMAL_BUILD_MODES:
            events = []

            def correctness_stub(*args):
                events.append(("correctness", args[5]))
                return correctness

            def batches_stub(*args):
                events.append(("batches", args[4], args[5]))
                return batches

            with mock.patch.object(
                benchmark, "correctness_gate", side_effect=correctness_stub
            ), mock.patch.object(
                benchmark,
                "collect_protocol_batches",
                side_effect=batches_stub,
            ):
                result = benchmark.protocol_result(
                    self.manifest,
                    self.suite_root,
                    {},
                    mock.Mock(),
                    build_mode,
                    authorization,
                )
            self.assertEqual(
                events,
                [
                    ("correctness", build_mode),
                    ("batches", build_mode, authorization),
                ],
            )
            self.assertEqual(result["status"], "completed")

    def test_raw_sample_recomputation_rejects_tampered_summaries(self) -> None:
        result = self.completed_result()
        protocol = result["protocols"]["release"]
        authorization = result["provenance"]["environment_qualification"]
        recorded_suite_root = benchmark.artifact_path(
            result["provenance"]["manifest_path"], "Linux"
        ).parent
        benchmark.validate_protocol(
            protocol,
            "release",
            self.manifest,
            result["builds"],
            result["provenance"]["collector"]["id"],
            authorization,
            "Linux",
            "x86_64",
            result["provenance"]["toolchains"]["runtime_environments"],
            recorded_suite_root,
            live_authority=False,
        )
        changed = copy.deepcopy(protocol)
        changed["batches"][0]["evaluation"]["suite"]["comparisons"]["c"][
            "point_ratio"
        ] = 0.5
        with self.assertRaisesRegex(benchmark.HarnessError, "recomputation"):
            benchmark.validate_protocol(
                changed,
                "release",
                self.manifest,
                result["builds"],
                result["provenance"]["collector"]["id"],
                authorization,
                "Linux",
                "x86_64",
                result["provenance"]["toolchains"]["runtime_environments"],
                recorded_suite_root,
                live_authority=False,
            )
        changed = copy.deepcopy(protocol)
        changed["batches"][0]["stability"]["valid"] = False
        with self.assertRaisesRegex(benchmark.HarnessError, "recomputation"):
            benchmark.validate_protocol(
                changed,
                "release",
                self.manifest,
                result["builds"],
                result["provenance"]["collector"]["id"],
                authorization,
                "Linux",
                "x86_64",
                result["provenance"]["toolchains"]["runtime_environments"],
                recorded_suite_root,
                live_authority=False,
            )

    def test_dynamic_snapshot_and_pair_issues_are_recomputed(self) -> None:
        result = self.completed_result()
        protocol = copy.deepcopy(result["protocols"]["release"])
        authorization = result["provenance"]["environment_qualification"]
        recorded_suite_root = benchmark.artifact_path(
            result["provenance"]["manifest_path"], "Linux"
        ).parent
        after = protocol["batches"][0]["dynamic_environment_after"]
        load = after["observations"]["concurrent_load"]
        load["status"] = "failed"
        load["parsed"]["one_minute_per_logical_core"] = 99.0
        body = {
            key: value
            for key, value in after.items()
            if key != "snapshot_sha256"
        }
        after["snapshot_sha256"] = benchmark.canonical_json_sha256(body)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "does not match raw content"
        ):
            benchmark.validate_protocol(
                protocol,
                "release",
                self.manifest,
                result["builds"],
                result["provenance"]["collector"]["id"],
                authorization,
                "Linux",
                "x86_64",
                result["provenance"]["toolchains"]["runtime_environments"],
                recorded_suite_root,
                live_authority=False,
            )

        protocol = copy.deepcopy(result["protocols"]["release"])
        after = protocol["batches"][0]["dynamic_environment_after"]
        after["observations"]["affinity"]["parsed"]["cpus"] = [0]
        raw_text = "[0]"
        after["observations"]["affinity"]["raw"] = {
            "sha256": v1.sha256_bytes(raw_text.encode("utf-8")),
            "length_bytes": len(raw_text.encode("utf-8")),
            "text": raw_text,
        }
        body = {
            key: value
            for key, value in after.items()
            if key != "snapshot_sha256"
        }
        after["snapshot_sha256"] = benchmark.canonical_json_sha256(body)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "completed batch must have"
        ):
            benchmark.validate_protocol(
                protocol,
                "release",
                self.manifest,
                result["builds"],
                result["provenance"]["collector"]["id"],
                authorization,
                "Linux",
                "x86_64",
                result["provenance"]["toolchains"]["runtime_environments"],
                recorded_suite_root,
                live_authority=False,
            )

    def test_dynamic_command_identity_and_locale_are_fail_closed(self) -> None:
        executable_name = "cmd.exe" if os.name == "nt" else "sh"
        executable = shutil.which(
            executable_name, path=benchmark.stable_build_path()
        )
        self.assertIsNotNone(executable)
        command = (
            [str(executable), "/c", "ver"]
            if os.name == "nt"
            else [str(executable), "-c", "true"]
        )
        captured = benchmark._dynamic_command(
            command
        )
        benchmark.validate_dynamic_command_evidence(captured)
        self.assertTrue(Path(captured["command_argv"][0]).is_absolute())
        self.assertEqual(
            captured["environment"]["LC_ALL"], "C"
        )
        for field in ("environment", "sha256"):
            changed = copy.deepcopy(captured)
            if field == "environment":
                changed["environment"]["LC_ALL"] = "en_US.UTF-8"
            else:
                changed["command_identity"]["sha256"] = "0" * 64
            with self.subTest(field=field):
                with self.assertRaises(benchmark.HarnessError):
                    benchmark.validate_dynamic_command_evidence(changed)

    def test_darwin_foundation_thermal_requires_nominal_state(self) -> None:
        def observation(text: str) -> dict:
            return {
                "source": "command",
                "command_argv": [
                    benchmark.DARWIN_OSASCRIPT,
                    "-l",
                    "JavaScript",
                    "-e",
                    benchmark.DARWIN_THERMAL_STATE_SCRIPT,
                ],
                "raw": {
                    "text": text,
                    "sha256": v1.sha256_bytes(text.encode()),
                    "length_bytes": len(text.encode()),
                },
            }

        nominal = benchmark.parse_dynamic_observation_from_raw(
            "thermal_state",
            observation("0\n"),
            benchmark.DYNAMIC_ENVIRONMENT_POLICY,
        )
        unavailable = benchmark.parse_dynamic_observation_from_raw(
            "thermal_state",
            observation(""),
            benchmark.DYNAMIC_ENVIRONMENT_POLICY,
        )
        self.assertEqual(nominal["thermal_state_name"], "nominal")
        self.assertTrue(nominal["normal"])
        self.assertIsNone(unavailable["thermal_state"])
        for value, name in ((1, "fair"), (2, "serious"), (3, "critical")):
            degraded = benchmark.parse_dynamic_observation_from_raw(
                "thermal_state",
                observation(f"{value}\n"),
                benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            )
            with self.subTest(state=name):
                self.assertEqual(degraded["thermal_state_name"], name)
                self.assertFalse(degraded["normal"])
                self.assertFalse(
                    benchmark.dynamic_observation_is_qualified(
                        "thermal_state",
                        {"parsed": degraded},
                        benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                    )
                )

    def test_darwin_frequency_is_not_applicable_without_fake_percent(self) -> None:
        text = "\n".join(benchmark.DARWIN_PMSET_NO_RECORDED_LINES) + "\n"
        observation = {
            "source": "command",
            "command_argv": [benchmark.DARWIN_PMSET, "-g", "therm"],
            "raw": {
                "text": text,
                "sha256": v1.sha256_bytes(text.encode()),
                "length_bytes": len(text.encode()),
            },
        }
        parsed = benchmark.parse_dynamic_observation_from_raw(
            "frequency_governor",
            observation,
            benchmark.DYNAMIC_ENVIRONMENT_POLICY,
        )
        self.assertEqual(parsed["applicability"], "not-applicable")
        self.assertEqual(
            parsed["auxiliary_pmset"]["shape"],
            "complete-no-recorded",
        )
        self.assertNotIn("cpu_speed_limit_percent", parsed)
        self.assertTrue(
            benchmark.dynamic_observation_is_qualified(
                "frequency_governor",
                {"parsed": parsed},
                benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                "Darwin",
                "arm64",
            )
        )
        for degraded in (
            "",
            "garbage\n",
            f"{benchmark.DARWIN_PMSET_NO_RECORDED_LINES[0]}\n",
            text + f"{benchmark.DARWIN_PMSET_NO_RECORDED_LINES[0]}\n",
            text + "garbage\n",
            (
                "CPU_Speed_Limit = 100\n"
                "CPU_Scheduler_Limit = 100\n"
                "CPU_Available_CPUs = 11\n"
            ),
            "CPU_Speed_Limit = 50\nCPU_Scheduler_Limit = 50\n",
            "CPU_Speed_Limit = 150\nCPU_Scheduler_Limit = 100\n",
            (
                "Error\nCPU_Speed_Limit = 100\n"
                "CPU_Scheduler_Limit = 100\nCPU_Available_CPUs = 11\n"
            ),
            "Thermal warning level = 1\n",
            "Performance warning level = 1\n",
        ):
            changed = copy.deepcopy(observation)
            changed["raw"] = benchmark._raw_text_evidence(degraded)
            parsed_degraded = benchmark.parse_dynamic_observation_from_raw(
                "frequency_governor",
                changed,
                benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            )
            with self.subTest(raw=degraded):
                self.assertTrue(
                    parsed_degraded["auxiliary_pmset"][
                        "explicit_degradation"
                    ]
                )
                self.assertFalse(
                    benchmark.dynamic_observation_is_qualified(
                        "frequency_governor",
                        {"parsed": parsed_degraded},
                        benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                        "Darwin",
                        "arm64",
                    )
                )

    def test_intel_darwin_artifact_cannot_claim_frequency_not_applicable(
        self,
    ) -> None:
        host = {
            "os": "Darwin",
            "architecture": "x86_64",
            "logical_core_count": 4,
        }
        authority_sha = benchmark.canonical_json_sha256(host)

        def command_capture(command):
            argv = [str(part) for part in command]
            if argv[0] == benchmark.DARWIN_OSASCRIPT:
                text = "0\n"
            elif argv[-2:] == ["-g", "batt"]:
                text = "Now drawing from 'AC Power'\n"
            elif argv[-2:] == ["-g", "therm"]:
                text = (
                    "\n".join(benchmark.DARWIN_PMSET_NO_RECORDED_LINES)
                    + "\n"
                )
            elif argv[-1:] == ["-g"]:
                text = " lowpowermode 0\n"
            else:
                text = "total = 0.00M used = 0.00M free = 0.00M\n"
            return {
                "status": "captured",
                "source": "command",
                "command_argv": argv,
                "command_identity": {
                    "path": argv[0],
                    "realpath": argv[0],
                    "sha256": "0" * 64,
                    "version_output": None,
                },
                "environment": benchmark.dynamic_command_environment(),
                "exit_code": 0,
                "raw": benchmark._raw_text_evidence(text),
            }

        with mock.patch.object(
            benchmark.sys, "platform", "darwin"
        ), mock.patch.object(
            benchmark, "_dynamic_command", side_effect=command_capture
        ):
            snapshot = benchmark.capture_dynamic_environment(
                authority_sha,
                benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                host_function=lambda: host,
            )
        frequency = snapshot["observations"]["frequency_governor"]
        self.assertEqual(frequency["status"], "failed")
        frequency["status"] = "qualified"
        snapshot["eligible"] = True
        snapshot["reason"] = "tampered Intel Darwin qualification"
        body = {
            key: value
            for key, value in snapshot.items()
            if key != "snapshot_sha256"
        }
        snapshot["snapshot_sha256"] = benchmark.canonical_json_sha256(body)
        authorization = self.static_authorization_stub()
        authorization["expected_bindings"][
            "authority_host_sha256"
        ] = authority_sha
        with mock.patch.object(
            benchmark, "validate_dynamic_command_evidence"
        ), self.assertRaisesRegex(
            benchmark.HarnessError, "status was not recomputed"
        ):
            benchmark.validate_dynamic_snapshot(
                snapshot,
                authorization,
                "Darwin",
                "x86_64",
            )

    def test_darwin_dynamic_command_rejects_path_shadow(self) -> None:
        def observation(path: str, arguments: list[str]) -> dict:
            return {
                "source": "command",
                "command_argv": [path, *arguments],
                "command_identity": {
                    "path": path,
                    "realpath": path,
                    "sha256": "0" * 64,
                    "version_output": None,
                },
            }

        profiles = (
            (
                "power_mode",
                benchmark.DARWIN_PMSET,
                ["-g", "batt"],
            ),
            (
                "frequency_governor",
                benchmark.DARWIN_PMSET,
                ["-g", "therm"],
            ),
            (
                "thermal_state",
                benchmark.DARWIN_OSASCRIPT,
                [
                    "-l",
                    "JavaScript",
                    "-e",
                    benchmark.DARWIN_THERMAL_STATE_SCRIPT,
                ],
            ),
            (
                "swap",
                benchmark.DARWIN_SYSCTL,
                ["-n", "vm.swapusage"],
            ),
        )
        for observation_id, trusted, arguments in profiles:
            shadow = self.fixture_path("shadow", Path(trusted).name)
            with self.subTest(observation=observation_id):
                self.assertTrue(
                    benchmark.dynamic_source_profile_is_allowed(
                        "Darwin",
                        observation_id,
                        observation(trusted, arguments),
                    )
                )
                self.assertFalse(
                    benchmark.dynamic_source_profile_is_allowed(
                        "Darwin",
                        observation_id,
                        observation(shadow, arguments),
                    )
                )

    def test_windows_dynamic_snapshot_replays_from_raw_winapi_evidence(
        self,
    ) -> None:
        authority_sha = "a" * 64
        snapshot = self.windows_dynamic_snapshot(
            authority_sha,
            1,
            "2026-07-28T00:00:05+00:00",
        )
        authorization = self.static_authorization_stub()
        authorization["expected_bindings"][
            "authority_host_sha256"
        ] = authority_sha
        benchmark.validate_dynamic_snapshot(
            snapshot,
            authorization,
            "Windows",
            "x86_64",
            live_authority=False,
        )
        changed = copy.deepcopy(snapshot)
        power = changed["observations"]["power_mode"]
        raw_value = json.loads(power["raw"]["text"])
        raw_value["ac_line_status"] = 0
        power["raw"] = benchmark._raw_json_evidence(raw_value)
        body = {
            key: value
            for key, value in changed.items()
            if key != "snapshot_sha256"
        }
        changed["snapshot_sha256"] = benchmark.canonical_json_sha256(body)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "does not match raw content"
        ):
            benchmark.validate_dynamic_snapshot(
                changed,
                authorization,
                "Windows",
                "x86_64",
                live_authority=False,
            )
        for observation_id, source in (
            ("power_mode", "command"),
            ("concurrent_load", "os.getloadavg"),
            ("swap", "GlobalMemoryStatusEx"),
            ("affinity", "system-api"),
        ):
            with self.subTest(
                observation_id=observation_id, source=source
            ):
                self.assertFalse(
                    benchmark.dynamic_source_profile_is_allowed(
                        "Windows",
                        observation_id,
                        {"source": source},
                        live_authority=False,
                    )
                )

    @unittest.skipUnless(os.name == "nt", "requires native WinAPI")
    def test_windows_dynamic_ctypes_layouts_and_capture_replay(self) -> None:
        self.assertEqual(
            benchmark.WindowsSystemPowerStatus.BatteryLifeTime.offset, 4
        )
        self.assertEqual(
            benchmark.WindowsProcessorPowerInformation.MhzLimit.offset, 12
        )
        self.assertEqual(benchmark.WindowsSystemInfo.PageSize.offset, 4)
        for capture in (
            benchmark.windows_system_power_status,
            benchmark.windows_processor_power_information,
            benchmark.windows_system_power_information,
            benchmark.windows_page_file_information,
            benchmark.windows_process_affinity,
        ):
            with self.subTest(capture=capture.__name__):
                self.assertIsNotNone(capture())

    @unittest.skipUnless(os.name == "nt", "requires native Visual Studio SDK")
    def test_windows_build_environment_discovers_sdk_without_parent_poison(
        self,
    ) -> None:
        poison = str(Path(self.temporary.name) / "poison")
        with mock.patch.dict(
            os.environ,
            {
                "INCLUDE": poison,
                "LIB": poison,
                "LIBPATH": poison,
                "CFLAGS": "-DPOISONED_PARENT",
                "ProgramData": poison,
                "ProgramFiles": poison,
                "ProgramFiles(x86)": poison,
                "ProgramW6432": poison,
                "COMSPEC": poison,
                "PATHEXT": ".POISON",
            },
        ):
            support = benchmark.canonical_windows_build_support(refresh=True)
            actual, projection = benchmark.sanitized_build_environment()
            self.assertNotEqual(actual["INCLUDE"], poison)
            self.assertNotEqual(actual["LIB"], poison)
            self.assertNotEqual(actual["LIBPATH"], poison)
            self.assertNotIn("CFLAGS", actual)
            self.assertIn("windows_toolchain", projection)
            authority = support["authority"]
            self.assertGreaterEqual(
                len(authority["installation_candidates"]), 1
            )
            self.assertEqual(
                Path(
                    authority["chosen_installation_json"][
                        "installationPath"
                    ]
                ).resolve(),
                Path(authority["installation_path"]),
            )
            raw_vswhere = base64.b64decode(
                authority["vswhere_stdout"]["base64"]
            )
            self.assertIsInstance(
                json.loads(raw_vswhere.decode("utf-8-sig")), list
            )
            self.assertEqual(
                authority["vswhere_stdout"]["length_bytes"],
                len(raw_vswhere),
            )
            self.assertEqual(
                authority["vswhere_stdout"]["sha256"],
                v1.sha256_bytes(raw_vswhere),
            )
            self.assertEqual(
                set(authority["llvm_tools"]),
                {"clang.exe", "clang++.exe"},
            )
            for identity in authority["llvm_tools"].values():
                self.assertTrue(Path(identity["path"]).is_file())
                self.assertEqual(
                    identity["sha256"],
                    v1.sha256_file(Path(identity["realpath"])),
                )
            self.assertEqual(
                set(authority["sdk_crt_markers"]),
                {
                    "ucrt_header",
                    "windows_header",
                    "sdk_version_header",
                    "vc_runtime_header",
                    "ucrt_library",
                    "kernel32_library",
                    "vc_runtime_library",
                },
            )
            for marker in authority["sdk_crt_markers"].values():
                self.assertEqual(
                    marker["sha256"],
                    v1.sha256_file(Path(marker["path"])),
                )
            self.assertEqual(
                set(authority["excluded_candidates"]),
                {"path", "include", "lib", "libpath"},
            )
            for exclusions in authority["excluded_candidates"].values():
                for exclusion in exclusions:
                    self.assertTrue(Path(exclusion["path"]).is_absolute())
                    self.assertTrue(exclusion["reason"])
            retained_paths = [
                *authority["path"],
                *authority["include"],
                *authority["lib"],
                *authority["libpath"],
            ]
            self.assertFalse(
                any(
                    "netfx" in value.casefold()
                    or "reference assemblies" in value.casefold()
                    for value in retained_paths
                )
            )
            for name in (
                "ProgramData",
                "ProgramFiles",
                "ProgramFiles(x86)",
                "ProgramW6432",
                "COMSPEC",
                "PATHEXT",
            ):
                self.assertNotEqual(actual[name], poison)
            source = Path(self.temporary.name) / "sdk-probe.c"
            binary = Path(self.temporary.name) / "sdk-probe.exe"
            source.write_text(
                "#include <ctype.h>\nint main(void) { return isdigit('1') ? 0 : 1; }\n",
                encoding="utf-8",
            )
            clang = benchmark.resolve_executable("clang", "Clang C")
            self.assertEqual(
                benchmark._windows_executable_from_path(
                    "clang.exe", actual["PATH"]
                ),
                Path(
                    authority["llvm_tools"]["clang.exe"]["path"]
                ).resolve(),
            )
            lookup = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    "import shutil; print(shutil.which('clang') or '')",
                ],
                env=actual,
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(
                Path(lookup.stdout.strip()).resolve(),
                Path(
                    authority["llvm_tools"]["clang.exe"]["path"]
                ).resolve(),
            )
            subprocess.run(
                [
                    str(clang),
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_C_FLAGS,
                    str(source),
                    "-o",
                    str(binary),
                ],
                env=actual,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [str(binary)],
                env=actual,
                check=True,
                capture_output=True,
            )
            cpp_source = Path(self.temporary.name) / "sdk-probe.cpp"
            cpp_binary = Path(self.temporary.name) / "sdk-probe-cpp.exe"
            cpp_source.write_text(
                "#include <array>\n"
                "int main() { std::array<int, 1> v{1}; return v[0] - 1; }\n",
                encoding="utf-8",
            )
            clangxx = benchmark.resolve_executable("clang++", "Clang C++")
            subprocess.run(
                [
                    str(clangxx),
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_CPP_FLAGS,
                    str(cpp_source),
                    "-o",
                    str(cpp_binary),
                ],
                env=actual,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [str(cpp_binary)],
                env=actual,
                check=True,
                capture_output=True,
            )
            nomo = benchmark.binary_path(
                benchmark.REPOSITORY_ROOT / "target" / "release",
                "nomo",
            )
            self.assertTrue(nomo.is_file())
            workload = self.manifest["workloads"][0]
            source_record = workload["sources"]["nomo"]
            nomo_project = Path(self.temporary.name) / "nomo-build-smoke"
            v1.copy_nomo_project(
                self.suite_root / source_record["path"],
                self.suite_root / source_record["project_manifest"],
                nomo_project,
            )
            subprocess.run(
                [str(nomo), "build", str(nomo_project)],
                cwd=benchmark.REPOSITORY_ROOT,
                env=actual,
                check=True,
                capture_output=True,
            )
        self.assertTrue(binary.is_file())
        self.assertTrue(cpp_binary.is_file())

    @unittest.skipUnless(os.name == "nt", "requires native Windows LLVM")
    def test_windows_release_backend_environment_finds_bound_clang(
        self,
    ) -> None:
        support = benchmark.canonical_windows_build_support()
        actual, projection = benchmark.sanitized_build_environment()
        authority = support["authority"]["llvm_tools"]["clang.exe"]
        lookup = subprocess.run(
            [
                sys.executable,
                "-c",
                "import shutil; print(shutil.which('clang') or '')",
            ],
            env=actual,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            Path(lookup.stdout.strip()).resolve(),
            Path(authority["path"]).resolve(),
        )
        self.assertEqual(
            projection["windows_toolchain"]["llvm_tools"]["clang.exe"],
            authority,
        )

    def test_windows_visual_studio_selection_is_deterministic(self) -> None:
        older = Path(self.temporary.name) / "VS" / "17.10"
        latest_b = Path(self.temporary.name) / "VS" / "17.14-b"
        latest_a = Path(self.temporary.name) / "VS" / "17.14-a"
        incomplete = Path(self.temporary.name) / "VS" / "18.0"
        for path in (older, latest_b, latest_a, incomplete):
            tools = path / "Common7" / "Tools"
            tools.mkdir(parents=True)
            (tools / "VsDevCmd.bat").write_text("@echo off\r\n", encoding="utf-8")
        installations = [
            {
                "installationPath": str(latest_b),
                "installationVersion": "17.14.1.0",
                "productId": "b",
                "catalog": {"productLine": "Dev17"},
                "installDate": "2026-07-20T00:00:00Z",
                "isComplete": True,
                "isLaunchable": True,
                "isPrerelease": False,
            },
            {
                "installationPath": str(older),
                "installationVersion": "17.10.9.0",
                "productId": "older",
                "catalog": {"productLine": "Dev17"},
                "installDate": "2026-07-19T00:00:00Z",
                "isComplete": True,
                "isLaunchable": True,
                "isPrerelease": False,
            },
            {
                "installationPath": str(incomplete),
                "installationVersion": "18.0.0.0",
                "productId": "incomplete",
                "catalog": {"productLine": "Dev17"},
                "installDate": "2026-07-21T00:00:00Z",
                "isComplete": False,
                "isLaunchable": True,
                "isPrerelease": True,
            },
            {
                "installationPath": str(latest_a),
                "installationVersion": "17.14.1.0",
                "productId": "a",
                "catalog": {"productLine": "Dev17"},
                "installDate": "2026-07-20T00:00:00Z",
                "isComplete": True,
                "isLaunchable": True,
                "isPrerelease": False,
            },
        ]
        selected, candidates, reason = (
            benchmark.select_visual_studio_installation(installations)
        )
        self.assertEqual(
            Path(selected["installationPath"]).resolve(),
            latest_a.resolve(),
        )
        self.assertEqual(len(candidates), 4)
        self.assertIn("installationVersion descending", reason)
        incomplete_record = next(
            item for item in candidates if item["product_id"] == "incomplete"
        )
        self.assertFalse(incomplete_record["eligible"])

    def test_windows_vsdevcmd_command_quotes_via_argument_boundaries(self) -> None:
        cmd = Path(r"C:\Windows\System32\cmd.exe")
        vsdevcmd = Path(
            r"C:\Program Files\Microsoft Visual Studio\2022"
            r"\Enterprise\Common7\Tools\VsDevCmd.bat"
        )
        argv = benchmark.windows_vsdevcmd_command(cmd, vsdevcmd, "amd64")
        self.assertEqual(
            argv,
            [
                str(cmd),
                "/d",
                "/c",
                "call",
                str(vsdevcmd),
                "-no_logo",
                "-arch=amd64",
                "-host_arch=amd64",
                "&&",
                "set",
            ],
        )
        command_line = subprocess.list2cmdline(argv)
        self.assertIn(f'call "{vsdevcmd}"', command_line)
        self.assertNotIn(r"\\\"", command_line)

    def test_windows_path_resolution_does_not_read_parent_pathext(self) -> None:
        root = Path(self.temporary.name)
        first = root / "first"
        second = root / "second"
        first.mkdir()
        second.mkdir()
        compiler = first / "cl.exe"
        compiler.write_bytes(b"bound-cl")
        with mock.patch.dict(os.environ, {"PATHEXT": ".POISON"}):
            self.assertEqual(
                benchmark._windows_executable_from_path(
                    "cl.exe",
                    os.pathsep.join((str(first), str(second))),
                ),
                compiler.resolve(),
            )
            bare_compiler = first / "clang.EXE"
            bare_compiler.write_bytes(b"bound-clang")
            self.assertEqual(
                benchmark._windows_executable_from_path(
                    "clang",
                    os.pathsep.join((str(first), str(second))),
                ),
                bare_compiler.resolve(),
            )
        (second / "cl.exe").write_bytes(b"ambiguous-cl")
        with self.assertRaisesRegex(
            benchmark.HarnessError, "exactly one cl.exe"
        ):
            benchmark._windows_executable_from_path(
                "cl.exe",
                os.pathsep.join((str(first), str(second))),
                require_unique=True,
            )

    def test_windows_sdk_candidates_filter_netfx_and_record_exclusions(
        self,
    ) -> None:
        root = Path(self.temporary.name)
        selected = root / "Visual Studio"
        retained = selected / "VC" / "Tools" / "include"
        netfx = root / "Windows Kits" / "NETFXSDK" / "4.8" / "Include"
        retained.mkdir(parents=True)
        netfx.mkdir(parents=True)
        missing = root / "Reference Assemblies"
        kept, excluded = benchmark._filter_paths_within_roots(
            [str(retained), str(netfx), str(missing)],
            [selected],
            "INCLUDE",
        )
        self.assertEqual(kept, [str(retained.resolve())])
        self.assertEqual(
            excluded,
            [
                {
                    "path": str(netfx.resolve()),
                    "reason": (
                        "outside selected VS VC and Windows SDK/UCRT roots"
                    ),
                },
                {
                    "path": str(missing.resolve()),
                    "reason": "directory is unavailable",
                },
            ],
        )
        empty, libpath_excluded = benchmark._filter_paths_within_roots(
            [str(netfx)],
            [selected],
            "LIBPATH",
            require_nonempty=False,
        )
        self.assertEqual(empty, [])
        self.assertEqual(libpath_excluded, excluded[:1])

    def test_current_platform_dynamic_capture_replays(self) -> None:
        host = benchmark.host_provenance()
        authority_sha = benchmark.canonical_json_sha256(host)
        snapshot = benchmark.capture_dynamic_environment(
            authority_sha,
            benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            host_function=lambda: host,
        )
        authorization = self.static_authorization_stub()
        authorization["expected_bindings"][
            "authority_host_sha256"
        ] = authority_sha
        benchmark.validate_dynamic_snapshot(
            snapshot,
            authorization,
            str(host["os"]),
            str(host["architecture"]),
        )

    def test_missing_environment_qualification_fails_closed(self) -> None:
        result = benchmark.environment_qualification(
            self.manifest, None, self.qualification_bindings()
        )
        self.assertFalse(result["eligible"])
        self.assertEqual(result["status"], "ineligible")
        self.assertEqual(
            result["missing_or_unqualified"],
            self.manifest["environment_qualification"]["required_checks"],
        )

    def test_environment_example_validates_draft_2020_12(self) -> None:
        path = self.suite_root / "environment-qualification.example.json"
        benchmark.validate_json_schema(
            json.loads(path.read_text(encoding="utf-8")),
            self.suite_root / "schema" / "environment-v2.schema.json",
            "environment example",
        )

    def test_environment_qualification_requires_every_bound_check(self) -> None:
        required = self.manifest["environment_qualification"]["required_checks"]
        checks = {
            check: self.qualified_check("recorded")
            for check in required
        }
        bindings = self.qualification_bindings()
        checks["canonical_host_identity"] = self.qualified_check(
            bindings["authority_host_sha256"]
        )
        checks["toolchain_identity"] = self.qualified_check(
            bindings["reference_toolchains_sha256"]
        )
        checks["frozen_source_lock"] = self.qualified_check(
            bindings["frozen_source_lock_sha256"]
        )
        checks[required[-1]]["status"] = "unavailable"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "environment.json"
            document = {
                "schema": 1,
                "canonical_host_id": "test-host",
                "captured_at_utc": "2026-07-28T00:00:00+00:00",
                "dynamic_policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                "bindings": bindings,
                "checks": checks,
            }
            path.write_text(json.dumps(document), encoding="utf-8")
            result = benchmark.environment_qualification(
                self.manifest, str(path), bindings
            )
            self.assertFalse(result["eligible"])
            self.assertEqual(result["missing_or_unqualified"], [required[-1]])
            checks[required[-1]]["status"] = "qualified"
            path.write_text(json.dumps(document), encoding="utf-8")
            result = benchmark.environment_qualification(
                self.manifest, str(path), bindings
            )
            self.assertTrue(result["eligible"])

    def test_environment_qualification_rejects_cross_host_binding(self) -> None:
        required = self.manifest["environment_qualification"]["required_checks"]
        bindings = self.qualification_bindings()
        document = {
            "schema": 1,
            "canonical_host_id": "test-host",
            "captured_at_utc": "2026-07-28T00:00:00+00:00",
            "dynamic_policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            "bindings": {**bindings, "authority_host_sha256": "f" * 64},
            "checks": {
                check: self.qualified_check("recorded")
                for check in required
            },
        }
        document["checks"]["canonical_host_identity"] = self.qualified_check(
            bindings["authority_host_sha256"]
        )
        document["checks"]["toolchain_identity"] = self.qualified_check(
            bindings["reference_toolchains_sha256"]
        )
        document["checks"]["frozen_source_lock"] = self.qualified_check(
            bindings["frozen_source_lock_sha256"]
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "environment.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            result = benchmark.environment_qualification(
                self.manifest, str(path), bindings
            )
        self.assertFalse(result["eligible"])
        self.assertEqual(result["binding_mismatches"], ["authority_host_sha256"])

    def test_toolchain_patch_mismatch_is_reported(self) -> None:
        with mock.patch.object(
            benchmark,
            "resolve_executable",
            return_value=Path(sys.executable),
        ), mock.patch.object(
            benchmark,
            "build_probe_output",
            side_effect=[
                ("nomo 0.0.0-20260721120555\n\nCommands:", {}),
                ("Apple clang version 21.0.0", {}),
                ("Apple clang version 21.0.0", {}),
                ("go version go1.25.11 darwin/arm64", {}),
                (self.temporary.name, {}),
            ],
        ), mock.patch.object(
            benchmark,
            "clang_selected_driver",
            side_effect=[
                {
                    "path": sys.executable,
                    "invocation_realpath": sys.executable,
                    "invocation_sha256": "1" * 64,
                    "selected_path": sys.executable,
                    "realpath": sys.executable,
                    "sha256": "1" * 64,
                    "selection_command": None,
                },
                {
                    "path": sys.executable,
                    "invocation_realpath": sys.executable,
                    "invocation_sha256": "1" * 64,
                    "selected_path": sys.executable,
                    "realpath": sys.executable,
                    "sha256": "1" * 64,
                    "selection_command": None,
                },
            ],
        ), mock.patch.object(
            benchmark,
            "clang_target",
            side_effect=[
                ("arm64-apple-darwin", self.full_command([sys.executable, "-print-target-triple"])),
                ("arm64-apple-darwin", self.full_command([sys.executable, "-print-target-triple"])),
            ],
        ):
            with self.assertRaisesRegex(
                benchmark.ToolchainMismatch,
                "Go expected go1.25.12, found go1.25.11",
            ):
                benchmark.inspect_toolchains(
                    self.manifest,
                    sys.executable,
                    sys.executable,
                    sys.executable,
                    sys.executable,
                )

        with mock.patch.object(
            benchmark,
            "resolve_executable",
            return_value=Path(sys.executable),
        ), mock.patch.object(
            benchmark,
            "build_probe_output",
            side_effect=[
                ("nomo 0.0.0-20260721120555\n\nCommands:", {}),
                ("Apple clang version 21.0.0", {}),
                ("Apple clang version 21.0.0", {}),
                ("go version go1.25.12 darwin/arm64", {}),
                (self.temporary.name, {}),
            ],
        ), mock.patch.object(
            benchmark,
            "clang_selected_driver",
            return_value={
                "path": sys.executable,
                "invocation_realpath": sys.executable,
                "invocation_sha256": "1" * 64,
                "selected_path": sys.executable,
                "realpath": sys.executable,
                "sha256": "1" * 64,
                "selection_command": None,
            },
        ), mock.patch.object(
            benchmark,
            "clang_target",
            side_effect=[
                ("arm64-apple-darwin", self.full_command([sys.executable])),
                ("arm64-apple-darwin", self.full_command([sys.executable])),
            ],
        ):
            with self.assertRaisesRegex(
                benchmark.ToolchainMismatch,
                "driver invocation paths must be distinct",
            ):
                benchmark.inspect_toolchains(
                    self.manifest,
                    sys.executable,
                    sys.executable,
                    sys.executable,
                    sys.executable,
                )

    @unittest.skipIf(os.name == "nt", "requires POSIX symlink argv0 semantics")
    def test_executable_resolution_preserves_clang_driver_argv0(self) -> None:
        system_clang = shutil.which("clang")
        if system_clang is None:
            self.skipTest("Clang is unavailable")
        root = Path(self.temporary.name)
        clang = root / "clang"
        clangxx = root / "clang++"
        clang.symlink_to(Path(system_clang))
        clangxx.symlink_to(Path(system_clang))
        selected_c = benchmark.resolve_executable(str(clang), "Clang C")
        selected_cpp = benchmark.resolve_executable(
            str(clangxx), "Clang C++"
        )
        self.assertEqual(selected_c, clang.absolute())
        self.assertEqual(selected_cpp, clangxx.absolute())
        self.assertNotEqual(selected_c, selected_cpp)
        self.assertEqual(selected_c.resolve(), selected_cpp.resolve())
        source = root / "probe.cpp"
        binary = root / "probe"
        source.write_text(
            "#include <memory>\n"
            "int main() { auto p = std::make_unique<int[]>(1); return p[0]; }\n",
            encoding="utf-8",
        )
        subprocess.run(
            [
                str(selected_cpp),
                *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                "-std=c++20",
                str(source),
                "-o",
                str(binary),
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_rustup_multicall_paths_and_rustc_authority_are_bound(self) -> None:
        cargo_invocation = benchmark.resolve_executable("cargo", "Cargo")
        root = Path(self.temporary.name)
        resolution_environment = {
            "CARGO_TARGET_DIR": str((root / "target").resolve()),
            "CARGO_HOME": str((root / "cargo-home").resolve()),
        }
        cargo, rustc, resolution = benchmark.resolve_rustup_toolchain(
            cargo_invocation,
            root,
            resolution_environment,
        )
        self.assertEqual(cargo.name.lower().removesuffix(".exe"), "cargo")
        self.assertEqual(rustc.name.lower().removesuffix(".exe"), "rustc")
        self.assertNotEqual(cargo, rustc)
        self.assertEqual(cargo.parent, rustc.parent)
        self.assertEqual(resolution["selected_sysroot"], str(cargo.parent.parent))
        self.assertEqual(
            resolution["invocation_path"], str(cargo_invocation)
        )
        cargo_environment = {
            **resolution_environment,
            "RUSTC": str(rustc),
        }
        record = benchmark.rustc_authority(
            rustc,
            root,
            cargo_environment,
        )
        self.assertEqual(record["path"], str(rustc))
        self.assertEqual(record["realpath"], str(rustc.resolve()))
        self.assertEqual(record["version_fields"]["binary"], "rustc")
        self.assertTrue(Path(record["sysroot"]).is_absolute())
        self.assertTrue(record["driver_files"])
        self.assertEqual(
            record["version_command"]["environment"],
            benchmark.sanitized_build_environment(cargo_environment)[1],
        )

    @unittest.skipUnless(
        platform.system() == "Darwin", "requires the Apple xcrun shim"
    )
    def test_darwin_clang_selection_ignores_parent_developer_dir(self) -> None:
        poison = Path(self.temporary.name) / "invalid-developer"
        poison.mkdir()
        with mock.patch.dict(
            os.environ,
            {
                "DEVELOPER_DIR": str(poison),
                "TOOLCHAINS": "poison-toolchain",
            },
        ):
            clang = benchmark.resolve_executable("clang", "Clang C")
            identity = benchmark.clang_selected_driver(clang, "clang")
            output, version_command = benchmark.build_probe_output(
                clang,
                [*benchmark.CLANG_DRIVER_CONFIG_FLAGS, "--version"],
            )
        self.assertIn("clang version", output.lower())
        self.assertEqual(identity["path"], str(clang))
        self.assertTrue(Path(identity["selected_path"]).is_file())
        self.assertEqual(
            identity["sha256"],
            v1.sha256_file(Path(identity["realpath"])),
        )
        for command in (
            identity["selection_command"],
            version_command,
        ):
            self.assertIn(
                "DEVELOPER_DIR", command["environment"]["cleared"]
            )
            self.assertIn("TOOLCHAINS", command["environment"]["cleared"])
            self.assertNotIn(
                "DEVELOPER_DIR", command["environment"]["retained"]
            )
            self.assertNotIn("TOOLCHAINS", command["environment"]["retained"])

    @unittest.skipUnless(
        platform.system() == "Darwin", "requires the Apple SDK"
    )
    def test_darwin_build_environment_constructs_trusted_sdkroot(self) -> None:
        poison = self.fixture_path("poison-sdk")
        with mock.patch.dict(
            os.environ,
            {
                "SDKROOT": poison,
                "DEVELOPER_DIR": poison,
                "TOOLCHAINS": "poison",
            },
        ):
            support = benchmark.canonical_darwin_build_support(refresh=True)
            actual, projection = benchmark.sanitized_build_environment()
            self.assertEqual(actual["SDKROOT"], support["sdkroot"])
            self.assertNotEqual(actual["SDKROOT"], poison)
            self.assertEqual(projection["darwin_sdk"], support)
            self.assertIn("DEVELOPER_DIR", projection["cleared"])
            source = Path(self.temporary.name) / "sdk-probe.c"
            binary = Path(self.temporary.name) / "sdk-probe"
            source.write_text(
                "#include <TargetConditionals.h>\n"
                "int main(void) { return TARGET_OS_OSX ? 0 : 1; }\n",
                encoding="utf-8",
            )
            subprocess.run(
                [
                    str(benchmark.resolve_executable("clang", "Clang C")),
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_C_FLAGS,
                    str(source),
                    "-o",
                    str(binary),
                ],
                env=actual,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [str(binary)],
                env=actual,
                check=True,
                capture_output=True,
            )

    @unittest.skipUnless(hasattr(os, "wait4"), "requires POSIX wait4")
    def test_output_mismatch_and_timeout_are_rejected(self) -> None:
        collector = benchmark.PosixWait4Collector()
        with self.assertRaisesRegex(
            benchmark.SampleCollectionError, "output mismatch"
        ) as mismatch:
            collector.run(
                [sys.executable, "-c", "print('wrong')"],
                expected_stdout=b"right\n",
                timeout_seconds=5.0,
            )
        self.assertEqual(mismatch.exception.record["status"], "failed")
        self.assertEqual(
            mismatch.exception.record["failure_kind"], "output-mismatch"
        )
        self.assertFalse(mismatch.exception.record["timed_out"])
        self.assertIn("stderr", mismatch.exception.record)
        with self.assertRaisesRegex(
            benchmark.SampleTimeoutError, "was killed"
        ) as timeout:
            collector.run(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                expected_stdout=b"",
                timeout_seconds=0.02,
            )
        self.assertEqual(timeout.exception.record["failure_kind"], "timeout")
        self.assertTrue(timeout.exception.record["timed_out"])
        self.assertGreater(timeout.exception.record["wall_ns"], 0)

    @unittest.skipUnless(hasattr(os, "wait4"), "requires POSIX wait4")
    def test_runtime_environment_drops_parent_injection_from_child_and_record(
        self,
    ) -> None:
        injected = {
            "DYLD_INSERT_LIBRARIES": "/poison/dylib",
            "DYLD_LIBRARY_PATH": "/poison/dyld",
            "LD_PRELOAD": "/poison/preload.so",
            "LD_LIBRARY_PATH": "/poison/ld",
            "MALLOC_CONF": "junk:true",
            "MallocNanoZone": "0",
            "GODEBUG": "gctrace=1",
            "GOGC": "1",
            "NOMO_BENCHMARK_POISON": "1",
        }
        collector = benchmark.PosixWait4Collector()
        with mock.patch.dict(os.environ, injected):
            actual, recorded = benchmark._environment({})
            env_tool = shutil.which("env", path=benchmark.stable_build_path())
            if env_tool is None:
                self.skipTest("the system env executable is unavailable")
            probe = subprocess.run(
                [env_tool, "-0"],
                check=True,
                capture_output=True,
                env=actual,
            )
            sample, stdout = collector.run(
                [env_tool, "-0"],
                expected_stdout=probe.stdout,
                timeout_seconds=5.0,
            )
        observed = {}
        for item in stdout.rstrip(b"\0").split(b"\0"):
            name, value = item.decode("utf-8").split("=", 1)
            observed[name] = value
        self.assertEqual(observed, actual)
        self.assertEqual(actual, recorded)
        self.assertEqual(sample["environment"], recorded)
        for name in injected:
            self.assertNotIn(name, actual)
            self.assertNotIn(name, sample["environment"])
        go_actual, go_recorded = benchmark._environment(
            {"GOMAXPROCS": "1"}
        )
        self.assertEqual(go_actual, go_recorded)
        self.assertEqual(
            set(go_actual) - set(actual),
            {"GOMAXPROCS"},
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "limited to GOMAXPROCS=1"
        ):
            benchmark._environment({"GOGC": "off"})

    def test_windows_runtime_environment_uses_system_api_and_rejects_tamper(
        self,
    ) -> None:
        trusted_root = Path(self.temporary.name) / "trusted-windows"
        trusted_system = trusted_root / "System32"
        trusted_temp = trusted_root / "Temp"
        trusted_system.mkdir(parents=True)
        trusted_temp.mkdir()
        poisoned = {
            "SystemRoot": str(Path(self.temporary.name) / "fake-root"),
            "WINDIR": str(Path(self.temporary.name) / "fake-windir"),
            "TEMP": str(Path(self.temporary.name) / "fake-temp"),
            "TMP": str(Path(self.temporary.name) / "fake-tmp"),
            "LD_PRELOAD": "poison",
            "GOGC": "1",
            "NOMO_POISON": "1",
        }
        workload_id = benchmark.WORKLOAD_IDS[0]
        fixture_sha256 = self.manifest["workloads"][0]["fixtures"][
            "performance"
        ]["sha256"]
        binary = self.binary_record(workload_id, "release", "candidate")
        sample = self.measured_sample(
            wall_ns=100,
            phase="sample",
            build_mode="release",
            workload_id=workload_id,
            lane="candidate",
            input_value="5500",
            fixture_sha256=fixture_sha256,
            batch_index=1,
            attempt_index=1,
            order_position=1,
            block_index=1,
        )
        with mock.patch.object(
            benchmark.os, "name", "nt"
        ), mock.patch.object(
            benchmark,
            "windows_system_directory",
            return_value=trusted_system,
        ), mock.patch.dict(
            os.environ, poisoned
        ):
            actual, recorded = benchmark._environment({})
            self.assertEqual(actual, recorded)
            self.assertEqual(actual["SystemRoot"], str(trusted_root))
            self.assertEqual(actual["WINDIR"], str(trusted_root))
            self.assertEqual(actual["TEMP"], str(trusted_temp.resolve()))
            self.assertEqual(actual["TMP"], str(trusted_temp.resolve()))
            for name in ("LD_PRELOAD", "GOGC", "NOMO_POISON"):
                self.assertNotIn(name, actual)

        sample["environment"] = actual
        with mock.patch.object(
            benchmark, "_environment", return_value=(actual, actual)
        ):
            benchmark.validate_sample_binding(
                sample,
                binary,
                "5500",
                fixture_sha256,
                sample["collector"],
                "candidate",
            )
            for field in ("TEMP", "SystemRoot"):
                changed = copy.deepcopy(sample)
                changed["environment"][field] = str(
                    Path(self.temporary.name) / "tampered"
                )
                with self.subTest(field=field), self.assertRaisesRegex(
                    benchmark.HarnessError, "sample environment changed"
                ):
                    benchmark.validate_sample_binding(
                        changed,
                        binary,
                        "5500",
                        fixture_sha256,
                        sample["collector"],
                        "candidate",
                    )

    def test_stdout_normalization_is_crlf_to_lf_only(self) -> None:
        program = self.fixture_path("program")
        normalized = benchmark._validate_process_output(
            [program], 0, b"ok\r\n", b"", b"ok\n"
        )
        self.assertEqual(normalized, b"ok\n")
        with self.assertRaisesRegex(benchmark.HarnessError, "output mismatch"):
            benchmark._validate_process_output(
                [program], 0, b"ok \r\n", b"", b"ok\n"
            )

    def test_windows_api_signatures_are_pointer_width_safe(self) -> None:
        kernel32 = FakeLibrary(
            (
                "CreateJobObjectW",
                "SetInformationJobObject",
                "InitializeProcThreadAttributeList",
                "UpdateProcThreadAttribute",
                "DeleteProcThreadAttributeList",
                "CreateProcessW",
                "ResumeThread",
                "WaitForSingleObject",
                "TerminateJobObject",
                "TerminateProcess",
                "GetExitCodeProcess",
                "GetProcessTimes",
                "CloseHandle",
            )
        )
        psapi = FakeLibrary(("GetProcessMemoryInfo",))
        metadata = benchmark.configure_windows_api(kernel32, psapi)
        self.assertIs(kernel32.CreateJobObjectW.restype, benchmark.wintypes.HANDLE)
        self.assertIs(psapi.GetProcessMemoryInfo.restype, benchmark.wintypes.BOOL)
        self.assertEqual(
            kernel32.TerminateProcess.argtypes,
            [benchmark.wintypes.HANDLE, benchmark.wintypes.UINT],
        )
        self.assertIs(
            kernel32.UpdateProcThreadAttribute.restype,
            benchmark.wintypes.BOOL,
        )
        self.assertEqual(
            metadata["atomic_job_association"],
            "PROC_THREAD_ATTRIBUTE_JOB_LIST",
        )
        self.assertEqual(
            metadata["pointer_width_bits"], ctypes.sizeof(ctypes.c_void_p) * 8
        )
        self.assertEqual(
            metadata["handle_width_bits"], ctypes.sizeof(benchmark.wintypes.HANDLE) * 8
        )

    def test_windows_job_list_attribute_is_initialized_before_launch(
        self,
    ) -> None:
        kernel32 = mock.Mock()

        def initialize(attribute_list, count, flags, size_pointer):
            if attribute_list is None:
                size_pointer._obj.value = 128
                return False
            return True

        kernel32.InitializeProcThreadAttributeList.side_effect = initialize
        kernel32.UpdateProcThreadAttribute.return_value = True
        (
            storage,
            attribute_list,
            handles,
        ) = benchmark.initialize_windows_job_list_attribute(kernel32, 123)
        self.assertGreater(ctypes.sizeof(storage), 0)
        self.assertIsInstance(attribute_list, ctypes.c_void_p)
        self.assertEqual(len(handles), 1)
        update = kernel32.UpdateProcThreadAttribute.call_args.args
        self.assertEqual(update[2], 0x0002000D)
        self.assertEqual(update[4], ctypes.sizeof(benchmark.wintypes.HANDLE))
        kernel32.DeleteProcThreadAttributeList(attribute_list)

    def test_math_link_flag_matches_platform_driver_contract(self) -> None:
        workload = {"link_math": True}
        self.assertEqual(
            benchmark.math_flags_for_host(workload, "Windows"), []
        )
        for host_os in ("Darwin", "Linux"):
            self.assertEqual(
                benchmark.math_flags_for_host(workload, host_os), ["-lm"]
            )
        with mock.patch.object(
            benchmark.platform, "system", return_value="Windows"
        ):
            self.assertEqual(benchmark.math_flags(workload), [])

    @unittest.skipUnless(os.name == "nt", "actual Windows Job Object smoke")
    def test_windows_collector_runs_on_windows(self) -> None:
        sample, stdout = benchmark.WindowsJobObjectCollector().run(
            [
                sys.executable,
                "-c",
                "print('ok')",
            ],
            expected_stdout=b"ok\n",
            timeout_seconds=10.0,
        )
        self.assertEqual(stdout, b"ok\n")
        self.assertEqual(sample["stdout_normalization"], benchmark.STDOUT_NORMALIZATION)
        self.assertNotEqual(
            sample["stdout_raw_sha256"], sample["stdout_normalized_sha256"]
        )
        self.assertGreater(sample["wall_ns"], 0)
        self.assertGreaterEqual(sample["peak_rss_bytes"], 0)

    def test_release_capability_probes_build_subcommand_with_and_without_release(
        self,
    ) -> None:
        executable = Path(sys.executable)
        record = self.full_command([str(executable), "build", "--help"])
        with mock.patch.object(
            benchmark,
            "run_build_capture",
            return_value=(record, b"Usage: nomo build [--emit-c]\n", b""),
        ):
            result = benchmark.release_capability(executable, "candidate")
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(
            result["help_command"]["argv"],
            [str(executable), "build", "--help"],
        )
        with mock.patch.object(
            benchmark,
            "run_build_capture",
            return_value=(
                record,
                b"Usage: nomo build [OPTIONS]\n  --release  optimized\n",
                b"",
            ),
        ):
            result = benchmark.release_capability(executable, "candidate")
        self.assertEqual(result["status"], "available", result)

    def test_release_capable_driver_does_not_create_correctness_release_lanes(
        self,
    ) -> None:
        fixture = self.project_result_to_producer_os(
            self.correctness_only_result(), platform.system()
        )
        toolchains = fixture["provenance"]["toolchains"]
        collector = mock.Mock()
        collector.descriptor.return_value = fixture["provenance"]["collector"]
        reference_builds = [
            (
                fixture["builds"][workload]["references"],
                {},
            )
            for workload in benchmark.WORKLOAD_IDS
        ]
        available_probe = {
            "label": "current",
            "status": "available",
            "reason": "nomo build --help exposes --release",
            "emit_c_fallback_used": False,
            "help_command": self.full_command(
                [toolchains["nomo"]["path"], "build", "--help"]
            ),
        }
        output = Path(self.temporary.name) / "release-capable-correctness.json"
        manifest_path = self.manifest_path.resolve()
        with mock.patch.object(
            benchmark,
            "release_capability",
            return_value=available_probe,
        ) as capability_probe, mock.patch.object(
            benchmark,
            "build_reference_workload",
            side_effect=reference_builds,
        ), mock.patch.object(
            benchmark,
            "correctness_gate",
            return_value=fixture["correctness"],
        ):
            result = benchmark.run_correctness(
                Namespace(),
                self.manifest,
                manifest_path,
                manifest_path.parent,
                output,
                fixture["provenance"]["repository"],
                toolchains,
                collector,
                fixture["provenance"]["host"],
            )

        capability_probe.assert_not_called()
        self.assertEqual(result["provenance"]["host"]["os"], platform.system())
        self.assertEqual(
            result["provenance"]["manifest_path"], str(manifest_path)
        )
        self.assertTrue(
            benchmark.artifact_path_is_absolute(
                result["provenance"]["manifest_path"],
                result["provenance"]["host"]["os"],
            )
        )
        self.assertEqual(result["status"], "correctness-only")
        self.assertFalse(result["claims"]["claim_eligible"])
        self.assertEqual(result["overall_verdict"], "not_evaluated")
        for lane in ("candidate", "main"):
            self.assertEqual(
                result["release_lanes"][lane],
                {
                    "label": lane,
                    "status": "unavailable",
                    "reason": (
                        f"formal {lane} was not supplied in "
                        "correctness-only mode"
                    ),
                    "emit_c_fallback_used": False,
                },
            )
        self.assertTrue(
            all(
                protocol["verdict"] == "not_evaluated"
                for protocol in result["protocols"].values()
            )
        )
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(result, self.manifest)

    def test_stable_toolchain_identity_excludes_probe_telemetry(self) -> None:
        result = self.correctness_only_result()
        first = copy.deepcopy(result["provenance"]["toolchains"])
        second = copy.deepcopy(first)
        second["clang"]["target_command"]["duration_ns"] = 999999
        second["clang"]["target_command"]["stdout"] = "telemetry"
        second["clangxx"]["target_command"]["duration_ns"] = 888888
        self.assertEqual(
            benchmark.stable_toolchain_identity(first),
            benchmark.stable_toolchain_identity(second),
        )
        first_bindings = benchmark.qualification_bindings(
            result["provenance"]["host"],
            first,
            result["provenance"]["source_lock"],
            result["release_lanes"],
        )
        second_bindings = benchmark.qualification_bindings(
            result["provenance"]["host"],
            second,
            result["provenance"]["source_lock"],
            result["release_lanes"],
        )
        self.assertEqual(first_bindings, second_bindings)
        for tool, field, changed_value in (
            ("clang", "path", "/different/clang"),
            ("clang", "sha256", "0" * 64),
            ("clang", "version", "0.0.0"),
            ("clang", "target_triple", "different-target"),
            ("clang", "driver_config_flags", []),
        ):
            changed = copy.deepcopy(first)
            changed[tool][field] = changed_value
            self.assertNotEqual(
                benchmark.stable_toolchain_identity(first),
                benchmark.stable_toolchain_identity(changed),
            )
        changed = copy.deepcopy(first)
        changed["runtime_environments"]["default"]["TMPDIR"] = "/different/tmp"
        self.assertNotEqual(
            benchmark.stable_toolchain_identity(first),
            benchmark.stable_toolchain_identity(changed),
        )

    def test_protocol_outcome_combination_table(self) -> None:
        expected = {
            ("completed", "completed"): "completed",
            ("completed", "ineligible"): "ineligible",
            ("ineligible", "completed"): "ineligible",
            ("ineligible", "ineligible"): "ineligible",
            ("unavailable", "ineligible"): "ineligible",
            ("ineligible", "unavailable"): "ineligible",
            ("completed", "unavailable"): "unavailable",
            ("unavailable", "completed"): "unavailable",
            ("unavailable", "unavailable"): "unavailable",
        }
        for statuses, top_status in expected.items():
            protocols = {
                build_mode: {
                    "status": status,
                    "verdict": "pass" if status == "completed" else "not_evaluated",
                }
                for build_mode, status in zip(
                    benchmark.FORMAL_BUILD_MODES, statuses
                )
            }
            aggregate = benchmark.aggregate_protocol_outcome(protocols)
            self.assertEqual(aggregate["status"], top_status)
            self.assertEqual(
                aggregate["claim_eligible"],
                statuses == ("completed", "completed"),
            )

    def test_emit_c_capability_probes_build_subcommand(self) -> None:
        executable = Path(sys.executable)
        record = self.full_command([str(executable), "build", "--help"])
        with mock.patch.object(
            benchmark,
            "run_build_capture",
            return_value=(record, b"Usage: nomo build [--emit-c]\n", b""),
        ):
            result = benchmark.emit_c_capability(executable, "candidate")
        self.assertEqual(result["status"], "available")

    def test_missing_release_mode_is_unavailable_before_environment_gate(self) -> None:
        arguments = Namespace(
            candidate_commit="a" * 40,
            main_commit="b" * 40,
            candidate_checkout=self.fixture_path("candidate"),
            main_checkout=self.fixture_path("main"),
            cargo="cargo",
            environment_qualification=None,
            prepared_bundle=None,
        )
        states = []
        for lane, checkout, commit in (
            ("candidate", self.fixture_path("candidate"), "a" * 40),
            ("main", self.fixture_path("main"), "b" * 40),
        ):
            states.append(
                {
                    "label": lane,
                    "status": "available",
                    "reason": "fixture",
                    "emit_c_fallback_used": False,
                    "checkout": checkout,
                    "expected_commit": commit,
                    "nomo_sha256": "c" * 64,
                    "capabilities": {
                        "release": {"status": "unavailable"},
                        "emit-c": {"status": "available"},
                    },
                }
            )
        collector = mock.Mock()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host("Linux")
        )
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            benchmark, "release_lane_state", side_effect=states
        ):
            result = benchmark.run_prepare(
                arguments,
                self.manifest,
                self.manifest_path,
                self.suite_root,
                Path(temporary) / "result.json",
                {},
                {},
                collector,
                {},
            )
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["protocols"]["release"]["status"], "unavailable")
        self.assertFalse(result["claims"]["claim_eligible"])

    def test_release_lane_builds_compiler_and_binds_current_origin_main(self) -> None:
        commit = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            checkout = root / "checkout"
            (checkout / ".git").mkdir(parents=True)
            bundle = root / "bundle"
            repository = {"commit": commit, "dirty": False}
            sysroot = root / "fixture-rust-toolchain"
            (sysroot / "lib").mkdir(parents=True)
            driver = sysroot / "lib" / "librustc_driver-fixture"
            driver.write_bytes(b"fixture driver")
            resolution_environment = {
                "CARGO_TARGET_DIR": str(
                    (bundle / "compiler-build" / "main").resolve()
                ),
                "CARGO_HOME": str(
                    (
                        bundle
                        / "compiler-build"
                        / "main-cargo-home"
                    ).resolve()
                ),
            }
            rustup_resolution = {
                "kind": "rustup-which-v1",
                "invocation_path": sys.executable,
                "rustup": {
                    "path": sys.executable,
                    "realpath": str(Path(sys.executable).resolve()),
                    "sha256": v1.sha256_file(Path(sys.executable).resolve()),
                },
                "cargo_command": self.full_command(
                    [sys.executable, "which", "cargo"],
                    cwd=str(checkout.resolve()),
                    approved_environment_overrides=resolution_environment,
                ),
                "rustc_command": self.full_command(
                    [sys.executable, "which", "rustc"],
                    cwd=str(checkout.resolve()),
                    approved_environment_overrides=resolution_environment,
                ),
                "selected_sysroot": str(sysroot.resolve()),
            }
            rustc_record = {
                "path": sys.executable,
                "realpath": str(Path(sys.executable).resolve()),
                "sha256": v1.sha256_file(Path(sys.executable).resolve()),
                "version_output": (
                    "rustc 1.99.0 (012345678 2026-01-01)\n"
                    "binary: rustc\n"
                    f"commit-hash: {'0' * 40}\n"
                    "commit-date: 2026-01-01\n"
                    "host: fixture-target\n"
                    "release: 1.99.0\n"
                    "LLVM version: 22.0.0"
                ),
                "version_fields": {
                    "version": "rustc 1.99.0 (012345678 2026-01-01)",
                    "binary": "rustc",
                    "commit_hash": "0" * 40,
                    "commit_date": "2026-01-01",
                    "host": "fixture-target",
                    "release": "1.99.0",
                    "llvm_version": "22.0.0",
                },
                "version_command": self.full_command(
                    [sys.executable, "-vV"],
                    cwd=str(checkout.resolve()),
                    approved_environment_overrides={
                        **resolution_environment,
                        "RUSTC": sys.executable,
                    },
                ),
                "sysroot": str(sysroot.resolve()),
                "sysroot_command": self.full_command(
                    [sys.executable, "--print", "sysroot"],
                    cwd=str(checkout.resolve()),
                    approved_environment_overrides={
                        **resolution_environment,
                        "RUSTC": sys.executable,
                    },
                ),
                "toolchain": "fixture-rust-toolchain",
                "driver_files": [
                    {
                        "path": str(driver.resolve()),
                        "sha256": v1.sha256_file(driver),
                    }
                ],
            }

            def run_capture(
                command: list[str],
                timeout_seconds: float,
                cwd: Path,
                approved_environment_overrides: dict[str, str],
            ) -> tuple[dict, bytes, bytes]:
                target = Path(
                    approved_environment_overrides["CARGO_TARGET_DIR"]
                )
                cargo_home = Path(
                    approved_environment_overrides["CARGO_HOME"]
                )
                self.assertEqual(
                    cargo_home,
                    (
                        bundle
                        / "compiler-build"
                        / "main-cargo-home"
                    ).resolve(),
                )
                self.assertNotEqual(
                    cargo_home,
                    Path(os.environ.get("CARGO_HOME", "")).resolve(),
                )
                binary = benchmark.binary_path(target / "release", "nomo")
                binary.parent.mkdir(parents=True, exist_ok=True)
                binary.write_bytes(b"self-built-nomo")
                if command[-1:] == ["--version"]:
                    stdout = b"cargo 1.99.0\n"
                elif command[-1:] == ["-vV"]:
                    stdout = (
                        b"rustc 1.99.0 (012345678 2026-01-01)\n"
                        b"binary: rustc\n"
                        b"commit-hash: "
                        + b"0" * 40
                        + b"\ncommit-date: 2026-01-01\n"
                        b"host: fixture-target\n"
                        b"release: 1.99.0\n"
                        b"LLVM version: 22.0.0\n"
                    )
                elif command[-2:] == ["--print", "sysroot"]:
                    stdout = (
                        self.fixture_path("fixture-rust-toolchain").encode()
                        + b"\n"
                    )
                else:
                    stdout = b""
                return (
                    self.full_command(
                        command,
                        cwd=str(cwd),
                        approved_environment_overrides={
                            "CARGO_TARGET_DIR": str(target.resolve()),
                            "CARGO_HOME": str(cargo_home.resolve()),
                            "RUSTC": str(Path(sys.executable)),
                        },
                    ),
                    stdout,
                    b"",
                )

            def git_capture(_checkout: Path, command: list[str]) -> str:
                if command == ["remote", "get-url", "origin"]:
                    return "git@github.com:nomo-lang/nomo.git"
                if command == ["cat-file", "-e", f"{commit}^{{commit}}"]:
                    return ""
                if command == ["rev-parse", "--abbrev-ref", "HEAD"]:
                    return "HEAD"
                if command == ["ls-remote", "origin"]:
                    return f"{commit}\trefs/heads/main"
                if command == ["rev-parse", "origin/main"]:
                    return commit
                if command == ["ls-remote", "origin", "refs/heads/main"]:
                    return f"{commit}\trefs/heads/main"
                raise AssertionError(command)

            available_probe = {
                "label": "main",
                "status": "available",
                "reason": "fixture",
            }
            with mock.patch.object(
                v1, "repository_state", side_effect=[repository, repository]
            ), mock.patch.object(
                v1, "git_capture", side_effect=git_capture
            ), mock.patch.object(
                benchmark, "resolve_executable", return_value=Path(sys.executable)
            ), mock.patch.object(
                benchmark,
                "resolve_rustup_toolchain",
                return_value=(
                    Path(sys.executable),
                    Path(sys.executable),
                    rustup_resolution,
                ),
            ), mock.patch.object(
                benchmark,
                "rustc_authority",
                return_value=rustc_record,
            ), mock.patch.object(
                v1, "tool_version", return_value="cargo 1.99.0"
            ), mock.patch.object(
                benchmark, "run_build_capture", side_effect=run_capture
            ), mock.patch.object(
                benchmark, "release_capability", return_value=available_probe
            ), mock.patch.object(
                benchmark, "emit_c_capability", return_value=available_probe
            ), mock.patch.dict(
                os.environ,
                {
                    "CARGO_HOME": str(root / "poison-cargo-home"),
                },
            ):
                result = benchmark.release_lane_state(
                    str(checkout),
                    commit,
                    "main",
                    bundle,
                    600.0,
                    sys.executable,
                    True,
                )
        self.assertEqual(result["status"], "available", result)
        self.assertEqual(result["expected_commit"], commit)
        self.assertTrue(result["detached_head"])
        compiler = result["compiler_build"]
        self.assertEqual(compiler["origin_main_commit"], commit)
        self.assertEqual(compiler["remote_main_commit"], commit)
        self.assertIn("CARGO_TARGET_DIR", compiler["environment"])
        self.assertIn("CARGO_HOME", compiler["environment"])
        self.assertEqual(compiler["environment"]["RUSTC"], sys.executable)
        self.assertEqual(compiler["cargo_configs"], [])
        self.assertEqual(compiler["rustc"]["realpath"], str(Path(sys.executable).resolve()))
        self.assertEqual(compiler["rustc"]["toolchain"], "fixture-rust-toolchain")
        self.assertEqual(compiler["rustup_resolution"], rustup_resolution)
        self.assertEqual(
            compiler["command"]["argv"][1:],
            ["build", "--locked", "--release", "--bin", "nomo"],
        )

    def test_cargo_config_authority_rejects_external_ancestor_and_hashes_tracked(
        self,
    ) -> None:
        root = Path(self.temporary.name)
        checkout = root / "nested" / "checkout"
        (checkout / ".git").mkdir(parents=True)
        checked_in = checkout / ".cargo" / "config.toml"
        checked_in.parent.mkdir()
        checked_in.write_text(
            '[target.wasm32-unknown-unknown]\nrustflags = []\n',
            encoding="utf-8",
        )
        with mock.patch.object(
            v1,
            "git_capture",
            return_value=".cargo/config.toml",
        ):
            records = benchmark.cargo_config_provenance(checkout)
        self.assertEqual(
            records,
            [
                {
                    "path": str(checked_in.resolve()),
                    "relative_path": ".cargo/config.toml",
                    "sha256": v1.sha256_file(checked_in),
                }
            ],
        )

        checked_in.unlink()
        poison = root / ".cargo" / "config.toml"
        poison.parent.mkdir()
        poison.write_text(
            "[build]\nrustflags = ['-Ctarget-cpu=native']\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "external ancestor Cargo config"
        ):
            benchmark.cargo_config_provenance(checkout)

        poison.unlink()
        checked_in.write_text(
            "[build]\nrustc = "
            f"{json.dumps(str(self.fixture_path('unbound-rustc')))}\n",
            encoding="utf-8",
        )
        with mock.patch.object(
            v1,
            "git_capture",
            return_value=".cargo/config.toml",
        ), self.assertRaisesRegex(
            benchmark.HarnessError, "authority-bound Rust compiler"
        ):
            benchmark.cargo_config_provenance(checkout)

        marker = root / "wrapper-executed"
        wrapper = root / "poison-wrapper"
        wrapper.write_text(
            f"#!{sys.executable}\n"
            "from pathlib import Path\n"
            f"Path({str(marker)!r}).write_text('executed')\n",
            encoding="utf-8",
        )
        wrapper.chmod(0o755)
        checked_in.write_text(
            f'build."rustc-workspace-wrapper" = {json.dumps(str(wrapper))}\n',
            encoding="utf-8",
        )
        with mock.patch.object(
            v1,
            "git_capture",
            return_value=".cargo/config.toml",
        ), self.assertRaisesRegex(
            benchmark.HarnessError, "authority-bound Rust compiler"
        ):
            benchmark.cargo_config_provenance(checkout)
        self.assertFalse(marker.exists())

    def test_isolated_cargo_home_ignores_real_parent_poison_config(
        self,
    ) -> None:
        cargo = shutil.which("cargo")
        if cargo is None:
            self.skipTest("Cargo is unavailable")
        root = Path(self.temporary.name)
        project = root / "project"
        source = project / "src"
        source.mkdir(parents=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname = "cargo-home-probe"\n'
            'version = "0.0.0"\nedition = "2021"\n',
            encoding="utf-8",
        )
        (source / "main.rs").write_text(
            "fn main() {}\n", encoding="utf-8"
        )
        poison_home = root / "poison-cargo-home"
        poison_home.mkdir()
        (poison_home / "config.toml").write_text(
            '[build]\nrustflags = ["--definitely-invalid-rustflag"]\n',
            encoding="utf-8",
        )
        isolated_home = root / "isolated-cargo-home"
        target = root / "target"
        with mock.patch.dict(
            os.environ, {"CARGO_HOME": str(poison_home)}
        ):
            actual, projection = benchmark.sanitized_build_environment(
                {
                    "CARGO_HOME": str(isolated_home),
                    "CARGO_TARGET_DIR": str(target),
                }
            )
            subprocess.run(
                [cargo, "check", "--offline"],
                cwd=project,
                env=actual,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        self.assertEqual(
            actual["CARGO_HOME"], str(isolated_home)
        )
        self.assertEqual(
            projection["retained"]["CARGO_HOME"],
            str(isolated_home.resolve()),
        )
        self.assertNotIn("CARGO_HOME", projection["cleared"])

    def test_release_lane_rejects_wrong_commit_before_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            checkout = Path(temporary) / "checkout"
            (checkout / ".git").mkdir(parents=True)
            with mock.patch.object(
                v1,
                "repository_state",
                return_value={"commit": "b" * 40, "dirty": False},
            ), mock.patch.object(
                v1,
                "git_capture",
                side_effect=lambda _checkout, command: (
                    "git@github.com:nomo-lang/nomo.git"
                    if command == ["remote", "get-url", "origin"]
                    else ""
                    if command == ["cat-file", "-e", f"{'a' * 40}^{{commit}}"]
                    else "HEAD"
                ),
            ), mock.patch.object(v1, "run_capture") as run_capture:
                result = benchmark.release_lane_state(
                    str(checkout),
                    "a" * 40,
                    "candidate",
                    Path(temporary) / "bundle",
                    600.0,
                    "cargo",
                    False,
                )
        self.assertEqual(result["status"], "unavailable")
        self.assertIn("commit mismatch", result["reason"])
        run_capture.assert_not_called()

    def test_release_lane_rejects_fake_origin(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            checkout = Path(temporary) / "checkout"
            (checkout / ".git").mkdir(parents=True)
            with mock.patch.object(
                v1,
                "repository_state",
                return_value={"commit": "a" * 40, "dirty": False},
            ), mock.patch.object(
                v1,
                "git_capture",
                return_value=self.fixture_path("fake-origin"),
            ):
                result = benchmark.release_lane_state(
                    str(checkout),
                    "a" * 40,
                    "candidate",
                    Path(temporary) / "bundle",
                    600.0,
                    "cargo",
                    False,
                )
        self.assertEqual(result["status"], "unavailable")
        self.assertIn("official", result["reason"])

    def test_candidate_main_must_be_distinct_and_cli_accepts_no_binary_override(
        self,
    ) -> None:
        arguments = Namespace(
            candidate_commit="a" * 40,
            main_commit="a" * 40,
            candidate_checkout=self.fixture_path("candidate"),
            main_checkout=self.fixture_path("main"),
        )
        self.assertIn("different", benchmark.lane_pair_conflict(arguments))
        parsed = benchmark.parse_arguments([])
        self.assertFalse(hasattr(parsed, "candidate_nomo"))
        self.assertFalse(hasattr(parsed, "main_nomo"))

    def test_prepared_bundle_round_trip_and_exact_authority(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        loaded = benchmark.load_prepared_bundle(bundle)
        self.assertEqual(loaded, result)
        benchmark.validate_result_schema(result, self.result_schema_path)
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ):
            benchmark.validate_result(result, self.manifest)
        outer = Path(self.temporary.name) / "prepared-result.json"
        v1.write_result(outer, result)
        reloaded = v1.read_json(outer)
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ):
            benchmark.validate_result(reloaded, self.manifest)
        self.assertEqual(
            set(reloaded["builds"]), set(benchmark.WORKLOAD_IDS)
        )

    def test_prepared_bundle_rejects_result_structure_tampering(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        mutations = []
        changed_repository = copy.deepcopy(result)
        changed_repository["provenance"]["repository"] = {"commit": "0" * 40}
        mutations.append(changed_repository)
        missing_mode = copy.deepcopy(result)
        del missing_mode["builds"]["spectral-norm"]["modes"]["emit-c"]
        mutations.append(missing_mode)
        unavailable_lane = copy.deepcopy(result)
        unavailable_lane["release_lanes"]["candidate"]["status"] = "unavailable"
        mutations.append(unavailable_lane)
        for changed in mutations:
            with self.subTest(change=len(mutations)):
                with self.assertRaises(benchmark.HarnessError):
                    benchmark.validate_prepared_bundle_authority(
                        changed, bundle, require_exact_result=True
                    )

    def test_prepared_request_is_canonical_and_metadata_is_strict(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        request_path = bundle / "qualification-request.json"
        canonical_request = v1.read_json(request_path)
        for field, value in (
            ("kind", "downgraded-request"),
            ("required_checks", []),
            ("dynamic_policy", {}),
        ):
            changed = copy.deepcopy(canonical_request)
            changed[field] = value
            benchmark.write_canonical_json(request_path, changed)
            with self.subTest(request_field=field):
                with self.assertRaises(benchmark.HarnessError):
                    benchmark.validate_prepared_bundle_authority(
                        result, bundle, require_exact_result=True
                    )
            benchmark.write_canonical_json(request_path, canonical_request)

        metadata_path = bundle / "prepared-bundle.json"
        canonical_metadata = v1.read_json(metadata_path)
        metadata_mutations = []
        extra = copy.deepcopy(canonical_metadata)
        extra["unexpected"] = True
        metadata_mutations.append(extra)
        missing = copy.deepcopy(canonical_metadata)
        del missing["prepared_at_utc"]
        metadata_mutations.append(missing)
        invalid_time = copy.deepcopy(canonical_metadata)
        invalid_time["prepared_at_utc"] = "not-utc"
        metadata_mutations.append(invalid_time)
        changed_valid_time = copy.deepcopy(canonical_metadata)
        changed_valid_time["prepared_at_utc"] = (
            benchmark.parse_utc_timestamp(
                canonical_metadata["prepared_at_utc"]
            )
            + dt.timedelta(seconds=1)
        ).isoformat()
        metadata_mutations.append(changed_valid_time)
        for changed in metadata_mutations:
            benchmark.write_canonical_json(metadata_path, changed)
            with self.assertRaises(benchmark.HarnessError):
                benchmark.load_prepared_bundle(bundle)
        benchmark.write_canonical_json(metadata_path, canonical_metadata)
        self.assertEqual(benchmark.load_prepared_bundle(bundle), result)

    def test_strict_json_loader_covers_all_authority_surfaces(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        result_path = Path(self.temporary.name) / "result.json"
        v1.write_result(result_path, result)
        paths = {
            "manifest": self.manifest_path,
            "result": result_path,
            "metadata": bundle / "prepared-bundle.json",
            "qualification-request": bundle / "qualification-request.json",
            "release-sidecar": Path(
                result["builds"]["spectral-norm"]["modes"]["release"][
                    "candidate"
                ]["backend_provenance_path"]
            ),
        }
        for label, path in paths.items():
            duplicate = (
                '{"authority_duplicate_probe":1,'
                '"authority_duplicate_probe":2}'
            )
            probe = Path(self.temporary.name) / f"{label}.duplicate.json"
            probe.write_text(duplicate, encoding="utf-8")
            with self.subTest(surface=label), self.assertRaisesRegex(
                benchmark.HarnessError, "duplicate JSON object key"
            ):
                benchmark.read_json_strict(probe)

        metadata_path = paths["metadata"]
        metadata_bytes = metadata_path.read_bytes()
        metadata_path.write_bytes(
            metadata_bytes.replace(
                b"{\n", b'{\n  "schema": 1,\n', 1
            )
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "duplicate JSON object key"
        ):
            benchmark.load_prepared_bundle(bundle)
        metadata_path.write_bytes(metadata_bytes)

        request_path = paths["qualification-request"]
        request_bytes = request_path.read_bytes()
        request_path.write_bytes(
            request_bytes.replace(
                b"{\n", b'{\n  "schema": 1,\n', 1
            )
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "duplicate JSON object key"
        ):
            benchmark.load_prepared_bundle(bundle)
        request_path.write_bytes(request_bytes)

        duplicate_result = Path(self.temporary.name) / "duplicate-result.json"
        duplicate_result.write_text(
            '{"schema":2,"schema":2}\n', encoding="utf-8"
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "duplicate JSON object key"
        ):
            benchmark.validate_artifact_offline(
                str(duplicate_result), str(self.manifest_path)
            )

    def test_post_approval_control_and_sidecar_mutation_is_rejected(
        self,
    ) -> None:
        result, bundle = self.prepared_bundle_fixture()
        sidecar_path = Path(
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance_path"]
        )
        sidecar = benchmark.read_json_strict(sidecar_path)
        sidecar["complete_argv"] = False
        v1.write_result(sidecar_path, sidecar)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "added, removed, or changed"
        ):
            benchmark.load_prepared_bundle(bundle)

        # Re-authoring the envelope cannot make a sidecar disagree with the
        # embedded, schema-bound provenance record.
        result["builds"]["spectral-norm"]["modes"]["release"]["candidate"][
            "backend_provenance_sha256"
        ] = v1.sha256_file(sidecar_path)
        benchmark.write_prepared_bundle(result, bundle, self.manifest)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "differs from its hashed sidecar"
        ):
            benchmark.validate_prepared_bundle_authority(
                result, bundle, require_exact_result=True
            )

    def test_prepared_inventory_rejects_missing_and_extra_payload_files(
        self,
    ) -> None:
        result, bundle = self.prepared_bundle_fixture()
        extra = bundle / "unapproved-extra.bin"
        extra.write_bytes(b"unapproved")
        with self.assertRaisesRegex(
            benchmark.HarnessError, "added, removed, or changed"
        ):
            benchmark.load_prepared_bundle(bundle)
        extra.unlink()

        copied_source = Path(
            result["builds"]["n-body"]["modes"]["emit-c"]["main"][
                "project"
            ]["copied_source"]["path"]
        )
        copied_source.unlink()
        with self.assertRaisesRegex(
            benchmark.HarnessError, "added, removed, or changed"
        ):
            benchmark.load_prepared_bundle(bundle)

    def test_prepared_authority_rejects_external_symlink_aliases(
        self,
    ) -> None:
        result, bundle = self.prepared_bundle_fixture()
        inventory = benchmark.prepared_file_inventory(bundle)
        formal = result["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]
        binary = Path(formal["binary"]["path"])
        project = Path(formal["project"]["path"])

        external_binary_alias = (
            Path(self.temporary.name) / "external-binary-alias"
        )
        external_project_alias = (
            Path(self.temporary.name) / "external-project-alias"
        )
        try:
            external_binary_alias.symlink_to(binary)
            external_project_alias.symlink_to(project, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"symlink creation is unavailable: {error}")
        try:
            changed = copy.deepcopy(result)
            changed["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["binary"]["path"] = str(external_binary_alias)
            with self.assertRaisesRegex(
                benchmark.HarnessError, "outside|symlink|junction"
            ):
                benchmark.validate_prepared_bundle_files(
                    changed, bundle, inventory
                )

            changed = copy.deepcopy(result)
            changed["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["project"]["path"] = str(external_project_alias)
            with self.assertRaisesRegex(
                benchmark.HarnessError, "outside|symlink|junction"
            ):
                benchmark.validate_prepared_bundle_files(
                    changed, bundle, inventory
                )
        finally:
            external_binary_alias.unlink(missing_ok=True)
            external_project_alias.unlink(missing_ok=True)

    def test_prepared_authority_rejects_external_hardlinks(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        inventory = benchmark.prepared_file_inventory(bundle)
        binary = Path(
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["binary"]["path"]
        )
        external_hardlink = Path(self.temporary.name) / "external-hardlink"
        try:
            os.link(binary, external_hardlink)
        except OSError as error:
            self.skipTest(f"hard links are unavailable: {error}")
        try:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "multiple hard links"
            ):
                benchmark.prepared_file_inventory(bundle)
            with self.assertRaisesRegex(
                benchmark.HarnessError, "multiple hard links"
            ):
                benchmark.validate_prepared_bundle_files(
                    result, bundle, inventory
                )
        finally:
            external_hardlink.unlink(missing_ok=True)

    @unittest.skipUnless(os.name == "nt", "requires a native Windows junction")
    def test_prepared_authority_rejects_external_windows_junction(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        inventory = benchmark.prepared_file_inventory(bundle)
        project = Path(
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["project"]["path"]
        )
        junction = Path(self.temporary.name) / "external-project-junction"
        completed = subprocess.run(
            [
                str(benchmark.windows_system_directory() / "cmd.exe"),
                "/d",
                "/c",
                "mklink",
                "/J",
                str(junction),
                str(project),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            completed.stderr.decode("utf-8", errors="replace"),
        )
        try:
            changed = copy.deepcopy(result)
            changed["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["project"]["path"] = str(junction)
            with self.assertRaisesRegex(
                benchmark.HarnessError, "outside|junction"
            ):
                benchmark.validate_prepared_bundle_files(
                    changed, bundle, inventory
                )
        finally:
            junction.rmdir()

    def test_prepared_inventory_reserved_names_and_output_collision(self) -> None:
        root = Path(self.temporary.name) / "inventory"
        nested = root / "nested"
        nested.mkdir(parents=True)
        (nested / "prepared-bundle.json").write_text(
            "nested", encoding="utf-8"
        )
        (nested / "qualification-request.json").write_text(
            "nested", encoding="utf-8"
        )
        paths = {
            item["path"]
            for item in benchmark.prepared_file_inventory(root)
        }
        self.assertEqual(
            paths,
            {
                "nested/prepared-bundle.json",
                "nested/qualification-request.json",
            },
        )
        symlink = root / "symlink"
        try:
            symlink.symlink_to(nested / "prepared-bundle.json")
        except OSError as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(
            benchmark.HarnessError, "symlinks"
        ):
            benchmark.prepared_file_inventory(root)
        symlink.unlink()
        for output in (root, root / "result.json"):
            with self.assertRaisesRegex(
                benchmark.HarnessError, "outside"
            ):
                benchmark.validate_output_outside_bundle(output, root)

    def test_same_output_rerun_preserves_stale_bundle_and_executes_nothing(
        self,
    ) -> None:
        root = Path(self.temporary.name).resolve()
        for stale_relative, is_file in (
            (
                Path("bin")
                / benchmark.binary_path(Path(), "spectral-norm-c"),
                True,
            ),
            (
                Path("build")
                / "spectral-norm"
                / "references"
                / "nomo-baseline-project",
                False,
            ),
            (
                Path("build")
                / "spectral-norm"
                / "references"
                / "nomo-baseline-project"
                / "build"
                / "c"
                / "main.c",
                True,
            ),
        ):
            with self.subTest(stale_relative=stale_relative):
                case_root = root / str(len(list(root.iterdir())))
                output = case_root / "correctness.json"
                bundle = output.with_suffix("")
                stale = bundle / stale_relative
                if is_file:
                    stale.parent.mkdir(parents=True)
                    stale.write_bytes(b"stale-build-output")
                else:
                    stale.mkdir(parents=True)
                arguments = benchmark.parse_arguments(
                    [
                        "--mode",
                        "correctness",
                        "--output",
                        str(output),
                    ]
                )
                with mock.patch.object(
                    v1, "repository_state"
                ) as repository_state, mock.patch.object(
                    benchmark, "inspect_toolchains"
                ) as inspect_toolchains, mock.patch.object(
                    benchmark, "run_build_capture"
                ) as build_capture:
                    with self.assertRaisesRegex(
                        benchmark.HarnessError,
                        "correctness build bundle already exists",
                    ):
                        benchmark.run_suite(arguments)
                repository_state.assert_not_called()
                inspect_toolchains.assert_not_called()
                build_capture.assert_not_called()
                self.assertTrue(
                    stale.exists(), "prior evidence must not be removed"
                )
                self.assertFalse(output.exists())
                self.assertFalse(output.with_suffix(".log").exists())

        output = root / "prior.json"
        log = output.with_suffix(".log")
        output.write_text('{"prior": true}\n', encoding="utf-8")
        log.write_text("prior-log\n", encoding="utf-8")
        arguments = benchmark.parse_arguments(
            ["--mode", "correctness", "--output", str(output)]
        )
        with mock.patch.object(
            benchmark, "run_build_capture"
        ) as build_capture:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "result output already exists"
            ):
                benchmark.run_suite(arguments)
        build_capture.assert_not_called()
        self.assertEqual(output.read_text(encoding="utf-8"), '{"prior": true}\n')
        self.assertEqual(log.read_text(encoding="utf-8"), "prior-log\n")

    def test_preflight_rejects_extension_and_ancestor_path_collisions(
        self,
    ) -> None:
        root = Path(self.temporary.name).resolve()
        cases = (
            (
                "extensionless",
                [
                    "--mode",
                    "correctness",
                    "--output",
                    str(root / "extensionless"),
                ],
                "exact .json extension",
                (root / "extensionless",),
            ),
            (
                "log-output",
                [
                    "--mode",
                    "correctness",
                    "--output",
                    str(root / "sidecar.log"),
                ],
                "exact .json extension",
                (root / "sidecar.log",),
            ),
            (
                "output-inside-bundle",
                [
                    "--mode",
                    "prepare",
                    "--output",
                    str(root / "prepared" / "result.json"),
                    "--prepared-bundle",
                    str(root / "prepared"),
                ],
                "preflight path collision",
                (root / "prepared",),
            ),
            (
                "bundle-inside-output",
                [
                    "--mode",
                    "prepare",
                    "--output",
                    str(root / "result.json"),
                    "--prepared-bundle",
                    str(root / "result.json" / "bundle"),
                ],
                "preflight path collision",
                (
                    root / "result.json",
                    root / "result.log",
                ),
            ),
        )
        for name, argv, message, absent_paths in cases:
            with self.subTest(name=name):
                arguments = benchmark.parse_arguments(argv)
                with mock.patch.object(
                    v1, "repository_state"
                ) as repository_state, mock.patch.object(
                    benchmark, "inspect_toolchains"
                ) as inspect_toolchains, mock.patch.object(
                    benchmark, "run_build_capture"
                ) as build_capture:
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, message
                    ):
                        benchmark.run_suite(arguments)
                repository_state.assert_not_called()
                inspect_toolchains.assert_not_called()
                build_capture.assert_not_called()
                for path in absent_paths:
                    self.assertFalse(path.exists())

    def test_preflight_rejects_dangling_lexical_targets_without_execution(
        self,
    ) -> None:
        root = Path(self.temporary.name).resolve() / "lexical-links"
        root.mkdir()
        cases = []
        for name in ("output", "sidecar", "bundle", "parent"):
            case_root = root / name
            case_root.mkdir()
            missing = case_root / "missing-target"
            output = case_root / "result.json"
            prepared_bundle = None
            if name == "output":
                output.symlink_to(missing)
            elif name == "sidecar":
                output.with_suffix(".log").symlink_to(missing)
            elif name == "bundle":
                prepared_bundle = case_root / "prepared"
                prepared_bundle.symlink_to(missing, target_is_directory=True)
            else:
                linked_parent = case_root / "linked-parent"
                linked_parent.symlink_to(missing, target_is_directory=True)
                output = linked_parent / "result.json"
            argv = ["--mode", "correctness", "--output", str(output)]
            if prepared_bundle is not None:
                argv = [
                    "--mode",
                    "prepare",
                    "--output",
                    str(output),
                    "--prepared-bundle",
                    str(prepared_bundle),
                ]
            cases.append((name, argv, missing, output))

        for name, argv, missing, output in cases:
            with self.subTest(name=name):
                arguments = benchmark.parse_arguments(argv)
                with mock.patch.object(
                    v1, "repository_state"
                ) as repository_state, mock.patch.object(
                    benchmark, "inspect_toolchains"
                ) as inspect_toolchains, mock.patch.object(
                    benchmark, "run_build_capture"
                ) as build_capture:
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, "symlink|junction"
                    ):
                        benchmark.run_suite(arguments)
                repository_state.assert_not_called()
                inspect_toolchains.assert_not_called()
                build_capture.assert_not_called()
                self.assertFalse(missing.exists())
                if not output.is_symlink():
                    self.assertFalse(output.exists())
                self.assertEqual(
                    list(output.parent.glob(".nomo-benchmarksgame-v2-case-*")),
                    [],
                )

    def test_preflight_case_aliases_are_filesystem_aware_and_zero_execution(
        self,
    ) -> None:
        root = Path(self.temporary.name).resolve() / "case-aliases"
        root.mkdir()
        cases = (
            (
                "bundle-vs-sidecar",
                root / "result.json",
                root / "RESULT.LOG",
            ),
            (
                "request-vs-output",
                root / "bundle" / "QUALIFICATION-REQUEST.json",
                root / "BUNDLE",
            ),
        )

        def assert_rejected(
            output: Path,
            bundle: Path,
            *,
            force_case_insensitive: bool,
        ) -> None:
            arguments = benchmark.parse_arguments(
                [
                    "--mode",
                    "prepare",
                    "--output",
                    str(output),
                    "--prepared-bundle",
                    str(bundle),
                ]
            )
            case_patch = (
                mock.patch.object(
                    benchmark,
                    "filesystem_is_case_sensitive",
                    return_value=False,
                )
                if force_case_insensitive
                else contextlib.nullcontext()
            )
            with case_patch, mock.patch.object(
                v1, "repository_state"
            ) as repository_state, mock.patch.object(
                benchmark, "inspect_toolchains"
            ) as inspect_toolchains, mock.patch.object(
                benchmark, "run_build_capture"
            ) as build_capture:
                with self.assertRaisesRegex(
                    benchmark.HarnessError, "preflight path collision"
                ):
                    benchmark.run_suite(arguments)
            repository_state.assert_not_called()
            inspect_toolchains.assert_not_called()
            build_capture.assert_not_called()
            self.assertFalse(output.exists())
            self.assertFalse(output.with_suffix(".log").exists())
            self.assertFalse(bundle.exists())

        for name, output, bundle in cases:
            with self.subTest(name=name, contract="case-insensitive"):
                assert_rejected(
                    output, bundle, force_case_insensitive=True
                )

        if not benchmark.filesystem_is_case_sensitive(root / "native-probe"):
            for name, output, bundle in cases:
                with self.subTest(name=name, contract="native-filesystem"):
                    assert_rejected(
                        output, bundle, force_case_insensitive=False
                    )
        self.assertEqual(
            list(root.glob(".nomo-benchmarksgame-v2-case-*")), []
        )

    def test_preflight_unicode_normalization_aliases_are_zero_execution(
        self,
    ) -> None:
        root = Path(self.temporary.name).resolve() / "unicode-aliases"
        root.mkdir()

        def assert_rejected(output: Path, bundle: Path) -> None:
            arguments = benchmark.parse_arguments(
                [
                    "--mode",
                    "prepare",
                    "--output",
                    str(output),
                    "--prepared-bundle",
                    str(bundle),
                ]
            )
            with mock.patch.object(
                v1, "repository_state"
            ) as repository_state, mock.patch.object(
                benchmark, "inspect_toolchains"
            ) as inspect_toolchains, mock.patch.object(
                benchmark, "run_build_capture"
            ) as build_capture:
                with self.assertRaisesRegex(
                    benchmark.HarnessError, "preflight path collision"
                ):
                    benchmark.run_suite(arguments)
            repository_state.assert_not_called()
            inspect_toolchains.assert_not_called()
            build_capture.assert_not_called()
            self.assertFalse(output.exists())
            self.assertFalse(output.with_suffix(".log").exists())
            self.assertFalse(bundle.exists())

        assert_rejected(
            root / "result-\N{LATIN SMALL LETTER E WITH ACUTE}.json",
            root / "result-e\u0301.log",
        )

        if platform.system() == "Darwin":
            composed_parent = root / "caf\N{LATIN SMALL LETTER E WITH ACUTE}"
            decomposed_parent = root / "cafe\u0301"
            composed_parent.mkdir()
            self.assertTrue(
                decomposed_parent.exists(),
                "the macOS benchmark authority filesystem must expose "
                "canonical Unicode aliases",
            )
            self.assertTrue(composed_parent.samefile(decomposed_parent))
            assert_rejected(
                composed_parent / "result.json",
                decomposed_parent / "result.log",
            )

        self.assertEqual(
            list(root.glob(".nomo-benchmarksgame-v2-case-*")), []
        )

    def native_windows_short_path(self, path: Path) -> Path:
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.GetShortPathNameW.argtypes = [
            benchmark.wintypes.LPCWSTR,
            benchmark.wintypes.LPWSTR,
            benchmark.wintypes.DWORD,
        ]
        kernel32.GetShortPathNameW.restype = benchmark.wintypes.DWORD
        buffer = ctypes.create_unicode_buffer(32768)
        length = int(
            kernel32.GetShortPathNameW(
                str(path),
                buffer,
                len(buffer),
            )
        )
        self.assertGreater(length, 0)
        self.assertLess(length, len(buffer))
        return Path(buffer.value)

    @unittest.skipUnless(os.name == "nt", "requires native Windows 8.3 aliases")
    def test_preflight_windows_short_name_alias_is_zero_execution(self) -> None:
        root = Path(self.temporary.name).resolve()
        short_root = self.native_windows_short_path(root)
        if os.path.normcase(str(short_root)) == os.path.normcase(str(root)):
            self.skipTest("the native volume did not expose an 8.3 alias")
        self.assertEqual(
            os.path.normcase(str(benchmark.windows_long_path_name(short_root))),
            os.path.normcase(str(root)),
        )

        long_bundle = root / "prepared-bundle"
        short_output = short_root / "prepared-bundle" / "result.json"
        arguments = benchmark.parse_arguments(
            [
                "--mode",
                "prepare",
                "--output",
                str(short_output),
                "--prepared-bundle",
                str(long_bundle),
            ]
        )
        with mock.patch.object(
            v1, "repository_state"
        ) as repository_state, mock.patch.object(
            benchmark, "inspect_toolchains"
        ) as inspect_toolchains, mock.patch.object(
            benchmark, "run_build_capture"
        ) as build_capture, mock.patch.object(
            benchmark, "load_prepared_bundle"
        ) as load_prepared_bundle:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "preflight path collision"
            ):
                benchmark.run_suite(arguments)
        repository_state.assert_not_called()
        inspect_toolchains.assert_not_called()
        build_capture.assert_not_called()
        load_prepared_bundle.assert_not_called()
        self.assertFalse(long_bundle.exists())
        self.assertFalse(short_output.exists())
        self.assertFalse(short_output.with_suffix(".log").exists())

    @unittest.skipUnless(os.name == "nt", "requires native Windows 8.3 aliases")
    def test_preflight_windows_short_bundle_alias_is_zero_execution(self) -> None:
        root = Path(self.temporary.name).resolve()
        long_bundle = root / "very-long-prepared-bundle-name"
        long_bundle.mkdir()
        (long_bundle / "prepared-bundle.json").write_text(
            "{}\n", encoding="utf-8"
        )
        (long_bundle / "qualification-request.json").write_text(
            "{}\n", encoding="utf-8"
        )
        short_bundle = self.native_windows_short_path(long_bundle)
        if os.path.normcase(str(short_bundle)) == os.path.normcase(
            str(long_bundle)
        ):
            self.skipTest("the native volume did not expose a final 8.3 alias")
        self.assertEqual(
            os.path.normcase(
                str(benchmark.windows_long_path_name(short_bundle))
            ),
            os.path.normcase(str(long_bundle)),
        )

        long_output = long_bundle / "result.json"
        arguments = benchmark.parse_arguments(
            [
                "--mode",
                "measure",
                "--output",
                str(long_output),
                "--prepared-bundle",
                str(short_bundle),
            ]
        )
        with mock.patch.object(
            v1, "repository_state"
        ) as repository_state, mock.patch.object(
            benchmark, "inspect_toolchains"
        ) as inspect_toolchains, mock.patch.object(
            benchmark, "run_build_capture"
        ) as build_capture, mock.patch.object(
            benchmark, "load_prepared_bundle"
        ) as load_prepared_bundle:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "preflight path collision"
            ):
                benchmark.run_suite(arguments)
        repository_state.assert_not_called()
        inspect_toolchains.assert_not_called()
        build_capture.assert_not_called()
        load_prepared_bundle.assert_not_called()
        self.assertFalse(long_output.exists())
        self.assertFalse(long_output.with_suffix(".log").exists())
        self.assertEqual(
            sorted(path.name for path in long_bundle.iterdir()),
            ["prepared-bundle.json", "qualification-request.json"],
        )

    def test_prepared_structure_and_live_sha_are_fail_closed(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        for mutation in ("lane", "mode"):
            changed = copy.deepcopy(result)
            if mutation == "lane":
                changed["release_lanes"]["candidate"][
                    "status"
                ] = "unavailable"
            else:
                del changed["builds"]["spectral-norm"]["modes"]["emit-c"]
            with self.subTest(mutation=mutation):
                with self.assertRaises(benchmark.HarnessError):
                    benchmark.validate_prepared_structure(changed)
        inventory = benchmark.prepared_file_inventory(bundle)
        binary_path = Path(
            result["builds"]["spectral-norm"]["references"]["binaries"][
                "c"
            ]["path"]
        )
        binary_path.write_bytes(b"tampered-after-prepare")
        with self.assertRaisesRegex(
            benchmark.HarnessError, "inventory and live file"
        ):
            benchmark.validate_prepared_bundle_files(
                result, bundle, inventory
            )

    def test_measure_preflight_rejects_rehashed_external_binary_without_execution(
        self,
    ) -> None:
        result, bundle = self.prepared_bundle_fixture()
        metadata_path = bundle / "prepared-bundle.json"
        metadata = v1.read_json(metadata_path)
        tokenized = metadata["prepared_result"]
        external = Path(sys.executable).resolve()
        external_sha = v1.sha256_file(external)
        reference = tokenized["builds"]["spectral-norm"]["references"]
        reference["binaries"]["c"] = {
            "path": str(external),
            "sha256": external_sha,
        }
        command = reference["commands"]["c_build"]
        output_index = command["argv"].index("-o") + 1
        command["argv"][output_index] = str(external)
        command["command"] = v1.command_text(command["argv"])
        materialized = benchmark.map_bundle_paths(
            tokenized, bundle, False
        )
        materialized_command = materialized["builds"]["spectral-norm"][
            "references"
        ]["commands"]["c_build"]
        materialized_command["command"] = v1.command_text(
            materialized_command["argv"]
        )
        digest = benchmark.prepared_bundle_digest(
            materialized,
            bundle,
            metadata["files"],
            metadata["prepared_at_utc"],
        )
        materialized["provenance"]["prepared_bundle_sha256"] = digest
        bindings = benchmark.qualification_bindings(
            materialized["provenance"]["host"],
            materialized["provenance"]["toolchains"],
            materialized["provenance"]["source_lock"],
            materialized["release_lanes"],
            digest,
        )
        materialized["provenance"]["environment_qualification"] = (
            benchmark.environment_qualification(
                self.manifest, None, bindings
            )
        )
        metadata["bundle_sha256"] = digest
        metadata["prepared_result"] = benchmark.map_bundle_paths(
            materialized, bundle, True
        )
        benchmark.write_canonical_json(metadata_path, metadata)
        benchmark.write_canonical_json(
            bundle / "qualification-request.json",
            benchmark.canonical_qualification_request(materialized),
        )
        collector = mock.Mock()
        output = Path(self.temporary.name) / "must-not-run.json"
        arguments = Namespace(
            prepared_bundle=str(bundle),
            environment_qualification=None,
        )
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ), mock.patch.object(
            benchmark, "run_build_capture"
        ) as build_capture:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "outside the prepared bundle"
            ):
                benchmark.run_measurement(
                    arguments,
                    self.manifest,
                    self.manifest_path,
                    self.suite_root,
                    output,
                    result["provenance"]["repository"],
                    result["provenance"]["toolchains"],
                    collector,
                    result["provenance"]["host"],
                )
        collector.run.assert_not_called()
        build_capture.assert_not_called()

    def test_prepare_approval_measure_orchestrator_consumes_bundle(self) -> None:
        prepared, bundle = self.prepared_bundle_fixture()
        request = v1.read_json(bundle / "qualification-request.json")
        checks = {
            check_id: self.qualified_check(f"qualified:{check_id}")
            for check_id in benchmark.EXPECTED_REQUIRED_CHECKS
        }
        checks["canonical_host_identity"] = self.qualified_check(
            request["bindings"]["authority_host_sha256"]
        )
        checks["toolchain_identity"] = self.qualified_check(
            request["bindings"]["reference_toolchains_sha256"]
        )
        checks["frozen_source_lock"] = self.qualified_check(
            request["bindings"]["frozen_source_lock_sha256"]
        )
        approval = {
            "schema": 1,
            "canonical_host_id": "fixture-host",
            "captured_at_utc": "2026-07-28T00:00:00+00:00",
            "dynamic_policy": request["dynamic_policy"],
            "bindings": request["bindings"],
            "checks": checks,
        }
        approval_path = Path(self.temporary.name) / "approval.json"
        v1.write_result(approval_path, approval)
        collector = mock.Mock()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host("Linux")
        )
        seen_modes = []

        def protocol_stub(
            _manifest,
            _suite_root,
            binaries,
            _collector,
            build_mode,
            static_authorization,
        ):
            seen_modes.append(build_mode)
            self.assertTrue(static_authorization["eligible"])
            self.assertEqual(set(binaries), set(benchmark.WORKLOAD_IDS))
            return {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": "orchestrator fixture stopped before timing",
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }

        arguments = Namespace(
            prepared_bundle=str(bundle),
            environment_qualification=str(approval_path),
        )
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ), mock.patch.object(
            benchmark, "protocol_result", side_effect=protocol_stub
        ):
            measured = benchmark.run_measurement(
                arguments,
                self.manifest,
                self.manifest_path,
                self.suite_root,
                Path(self.temporary.name) / "measured.json",
                prepared["provenance"]["repository"],
                prepared["provenance"]["toolchains"],
                collector,
                prepared["provenance"]["host"],
            )
        self.assertEqual(seen_modes, list(benchmark.FORMAL_BUILD_MODES))
        self.assertEqual(measured["status"], "unavailable")
        self.assertEqual(
            measured["provenance"]["prepared_bundle_sha256"],
            request["bundle_sha256"],
        )

    def test_build_environment_is_required_and_recomputed_everywhere(
        self,
    ) -> None:
        result = self.completed_result()
        commands = [
            result["builds"]["spectral-norm"]["references"]["commands"][
                "c_build"
            ],
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["command"],
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance"]["compile_commands"][0],
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance"]["link_command"],
            result["builds"]["spectral-norm"]["modes"]["emit-c"][
                "candidate"
            ]["emit_command"],
            result["builds"]["spectral-norm"]["modes"]["emit-c"][
                "candidate"
            ]["clang_command"],
        ]
        for command in commands:
            changed = copy.deepcopy(result)
            target_argv = command["argv"]

            def find_command(value):
                if isinstance(value, dict):
                    if value.get("argv") == target_argv:
                        return value
                    for nested_value in value.values():
                        found = find_command(nested_value)
                        if found is not None:
                            return found
                elif isinstance(value, list):
                    for nested_value in value:
                        found = find_command(nested_value)
                        if found is not None:
                            return found
                return None

            target = find_command(changed["builds"])
            self.assertIsNotNone(target)
            target["environment"] = {"tampered": True}
            with self.assertRaisesRegex(
                benchmark.HarnessError, "build environment|metadata"
            ):
                benchmark.validate_build_provenance(
                    changed, self.manifest
                )
        missing = copy.deepcopy(result)
        del missing["builds"]["spectral-norm"]["references"]["commands"][
            "c_build"
        ]["environment"]
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Draft 2020-12 schema"
        ):
            benchmark.validate_result_schema(
                missing, self.result_schema_path
            )
        missing = copy.deepcopy(result)
        del missing["provenance"]["toolchains"]["build_environment"]
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Draft 2020-12 schema"
        ):
            benchmark.validate_result_schema(
                missing, self.result_schema_path
            )
        changed = copy.deepcopy(result)
        changed["provenance"]["toolchains"]["runtime_environments"][
            "default"
        ]["PATH"] = "/poison/runtime-path"
        with self.assertRaisesRegex(
            benchmark.HarnessError, "runtime environment authority"
        ):
            benchmark.validate_result(changed, self.manifest)

        for field in ("compiler_build", "capability"):
            changed = copy.deepcopy(result)
            if field == "compiler_build":
                changed["release_lanes"]["candidate"]["compiler_build"][
                    "command"
                ]["environment"] = {"tampered": True}
            else:
                changed["release_lanes"]["candidate"]["capabilities"][
                    "release"
                ]["help_command"]["environment"] = {"tampered": True}
            with self.assertRaisesRegex(
                benchmark.HarnessError, "build environment"
            ):
                benchmark.validate_release_lane_authority(changed)

    def test_go_build_environment_disables_parent_and_user_go_env(
        self,
    ) -> None:
        go = shutil.which("go")
        if go is None:
            self.skipTest("Go is unavailable")
        poison = Path(self.temporary.name) / "go-env"
        poison.write_text("GOFLAGS=-x\n", encoding="utf-8")
        with mock.patch.dict(
            os.environ,
            {
                "GOENV": str(poison),
                "GOFLAGS": "-mod=vendor",
                "CARGO_HOME": str(
                    Path(self.temporary.name) / "poison-cargo-home"
                ),
            },
        ):
            actual, projection = benchmark.sanitized_build_environment()
            goenv = subprocess.run(
                [go, "env", "GOENV"],
                check=True,
                capture_output=True,
                text=True,
                env=actual,
            )
            goflags = subprocess.run(
                [go, "env", "GOFLAGS"],
                check=True,
                capture_output=True,
                text=True,
                env=actual,
            )
        self.assertEqual(actual["GOENV"], "off")
        self.assertNotIn("GOFLAGS", actual)
        self.assertNotIn("CARGO_HOME", actual)
        self.assertEqual(projection["retained"]["GOENV"], "off")
        self.assertNotIn("GOENV", projection["cleared"])
        self.assertIn("GOFLAGS", projection["cleared"])
        self.assertIn("CARGO_HOME", projection["cleared"])
        self.assertIn(goenv.stdout.strip(), {"", "off"})
        self.assertEqual(goflags.stdout.strip(), "")

    def test_go_build_uses_isolated_cache_without_parent_localappdata(
        self,
    ) -> None:
        go = shutil.which("go")
        if go is None:
            self.skipTest("Go is unavailable")
        root = Path(self.temporary.name) / "go-cache-smoke"
        root.mkdir()
        source = root / "main.go"
        source.write_text("package main\nfunc main() {}\n", encoding="utf-8")
        output = benchmark.binary_path(root, "go-cache-smoke")
        poison_cache = str((root / "parent-cache").resolve())
        with mock.patch.dict(
            os.environ,
            {
                "GOCACHE": poison_cache,
                "GOMODCACHE": str((root / "parent-module-cache").resolve()),
                "LocalAppData": str((root / "parent-local-app-data").resolve()),
            },
        ):
            os.environ.pop("LOCALAPPDATA", None)
            with benchmark.isolated_go_build_cache(root) as cache_environment:
                record, _, _ = benchmark.run_build_capture(
                    [go, "build", "-o", str(output), str(source)],
                    120.0,
                    approved_environment_overrides=cache_environment,
                )
                self.assertNotEqual(cache_environment["GOCACHE"], poison_cache)
                self.assertNotIn("LocalAppData", record["environment"]["retained"])
                self.assertNotIn("LOCALAPPDATA", record["environment"]["retained"])
                benchmark.validate_build_command_environment(
                    record,
                    "Go isolated-cache smoke",
                    cache_environment,
                )
        self.assertTrue(output.is_file())
        for directory in benchmark.GO_BUILD_CACHE_DIRECTORIES.values():
            self.assertFalse((root / directory).exists())

    def test_correctness_preflight_build_failure_is_written_and_validated(
        self,
    ) -> None:
        output = Path(self.temporary.name) / "correctness-build-failed.json"
        bundle = output.with_suffix("")
        source = bundle / "build" / "spectral-norm" / "references" / "go.go"
        source.parent.mkdir(parents=True)
        frozen_source = (
            self.suite_root
            / self.manifest["workloads"][0]["sources"]["go"]["path"]
        )
        shutil.copy2(frozen_source, source)
        binary = benchmark.binary_path(bundle / "bin", "spectral-norm-go")
        fixture = self.correctness_only_result()
        toolchains = fixture["provenance"]["toolchains"]
        self.bind_live_execution_authority(toolchains)
        cache_environment = {
            key: str((source.parent / directory).resolve())
            for key, directory in benchmark.GO_BUILD_CACHE_DIRECTORIES.items()
        }
        command_argv = [
            toolchains["go"]["path"],
            "build",
            "-o",
            str(binary.resolve()),
            str(source.resolve()),
        ]
        command = benchmark.failed_build_command_record(
            command_argv,
            REPOSITORY_ROOT,
            benchmark.sanitized_build_environment(cache_environment)[1],
            "2026-07-28T00:00:00+00:00",
            1,
            b"",
            b"build cache is required",
            exit_code=1,
            timed_out=False,
            error="command exited with status 1",
        )
        failure = benchmark.workload_build_failure_record(
            "spectral-norm",
            "go",
            "reference-build",
            source,
            binary,
            command,
        )
        collector = mock.Mock()
        host_os = platform.system()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host(host_os)
        )
        host = {"os": host_os, "architecture": platform.machine()}
        unavailable = {
            "status": "unavailable",
            "reason": "fixture release capability unavailable",
            "emit_c_fallback_used": False,
        }
        with mock.patch.object(
            benchmark, "release_capability", return_value=unavailable
        ), mock.patch.object(
            benchmark,
            "build_reference_workload",
            side_effect=benchmark.WorkloadBuildError("failed Go build", failure),
        ):
            result = benchmark.run_correctness(
                Namespace(),
                self.manifest,
                self.manifest_path,
                self.suite_root,
                output,
                {},
                toolchains,
                collector,
                host,
            )
        self.assertEqual(result["status"], "ineligible")
        self.assertEqual(result["correctness"], [])
        self.assertEqual(result["build_failures"], [failure])
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(result, self.manifest)
        v1.write_result(output, result)
        log_path = benchmark.write_evidence_log(output, result)
        reloaded = v1.read_json(output)
        benchmark.validate_result_schema(reloaded, self.result_schema_path)
        benchmark.validate_result(reloaded, self.manifest)
        self.assertIn("build cache is required", log_path.read_text())

    def assert_missing_output_evidence(self, missing_phase: str) -> None:
        output = (
            Path(self.temporary.name)
            / f"correctness-{missing_phase}-missing.json"
        ).resolve()
        bundle = output.with_suffix("")
        fixture = self.correctness_only_result()
        toolchains = fixture["provenance"]["toolchains"]
        self.bind_live_execution_authority(toolchains)

        def fake_capture(
            command: list[str],
            timeout_seconds: float,
            cwd: Path | None = None,
            approved_environment_overrides: dict[str, str] | None = None,
        ) -> tuple[dict, bytes, bytes]:
            del timeout_seconds
            argv = [str(part) for part in command]
            record = self.full_command(
                argv,
                cwd=str(cwd.resolve()) if cwd is not None else None,
                approved_environment_overrides=approved_environment_overrides,
            )
            is_go = argv[:2] == [toolchains["go"]["path"], "build"]
            is_emit = (
                argv[0] == toolchains["nomo"]["path"]
                and argv[-1] == "--emit-c"
            )
            is_baseline_clang = (
                argv[0] == toolchains["clang"]["path"]
                and "-o" in argv
                and "nomo-baseline"
                in Path(argv[argv.index("-o") + 1]).stem
            )
            should_omit = {
                "go": is_go,
                "emit-c": is_emit,
                "generated-c-clang": is_baseline_clang,
            }[missing_phase]
            if not should_omit:
                if is_emit:
                    generated = Path(argv[2]) / "build" / "c" / "main.c"
                    generated.parent.mkdir(parents=True, exist_ok=True)
                    generated.write_text(
                        "int main(void) { return 0; }\n", encoding="utf-8"
                    )
                elif "-o" in argv:
                    binary = Path(argv[argv.index("-o") + 1])
                    binary.parent.mkdir(parents=True, exist_ok=True)
                    binary.write_bytes(b"fixture-build-output")
            return record, b"", b""

        with mock.patch.object(
            benchmark, "run_build_capture", side_effect=fake_capture
        ):
            with self.assertRaises(benchmark.WorkloadBuildError) as captured:
                benchmark.build_reference_workload(
                    self.manifest["workloads"][0],
                    self.suite_root,
                    bundle,
                    toolchains,
                    120.0,
                    include_nomo_baseline=True,
                )
        failure = captured.exception.record
        self.assertEqual(failure["failure_kind"], "missing-output")
        self.assertEqual(failure["command"]["exit_code"], 0)
        self.assertFalse(Path(failure["output_path"]).exists())

        collector = mock.Mock()
        host_os = platform.system()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host(host_os)
        )
        host = {"os": host_os, "architecture": platform.machine()}
        unavailable = {
            "status": "unavailable",
            "reason": "fixture release capability unavailable",
            "emit_c_fallback_used": False,
        }
        with mock.patch.object(
            benchmark, "release_capability", return_value=unavailable
        ), mock.patch.object(
            benchmark,
            "build_reference_workload",
            side_effect=benchmark.WorkloadBuildError(
                str(captured.exception), failure
            ),
        ):
            result = benchmark.run_correctness(
                Namespace(),
                self.manifest,
                self.manifest_path,
                self.suite_root,
                output,
                {},
                toolchains,
                collector,
                host,
            )
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(result, self.manifest)
        v1.write_result(output, result)
        log_path = benchmark.write_evidence_log(output, result)
        benchmark.validate_result(v1.read_json(output), self.manifest)
        self.assertIn("expected build output is missing", log_path.read_text())
        with mock.patch.object(
            benchmark, "run_suite", return_value=(output, result)
        ):
            self.assertEqual(
                benchmark.main(["--mode", "correctness"]), 2
            )

    def test_fake_go_exit_zero_without_output_is_retained(self) -> None:
        self.assert_missing_output_evidence("go")

    def test_emit_c_exit_zero_without_generated_c_is_retained(self) -> None:
        self.assert_missing_output_evidence("emit-c")

    def test_generated_c_clang_exit_zero_without_binary_is_retained(self) -> None:
        self.assert_missing_output_evidence("generated-c-clang")

    def assert_invalid_output_evidence(self, output_kind: str) -> None:
        output = (
            Path(self.temporary.name)
            / f"correctness-go-{output_kind}.json"
        ).resolve()
        bundle = output.with_suffix("")
        fixture = self.correctness_only_result()
        toolchains = fixture["provenance"]["toolchains"]
        self.bind_live_execution_authority(toolchains)

        def fake_capture(
            command: list[str],
            timeout_seconds: float,
            cwd: Path | None = None,
            approved_environment_overrides: dict[str, str] | None = None,
        ) -> tuple[dict, bytes, bytes]:
            del timeout_seconds
            argv = [str(part) for part in command]
            record = self.full_command(
                argv,
                cwd=str(cwd.resolve()) if cwd is not None else None,
                approved_environment_overrides=approved_environment_overrides,
            )
            target = Path(argv[argv.index("-o") + 1])
            target.parent.mkdir(parents=True, exist_ok=True)
            is_go = argv[:2] == [toolchains["go"]["path"], "build"]
            if not is_go:
                target.write_bytes(b"fixture-build-output")
            elif output_kind == "directory":
                target.mkdir()
            elif output_kind == "symlink":
                symlink_target = target.parent / "symlink-target"
                symlink_target.write_bytes(b"not-this-build")
                target.symlink_to(symlink_target)
            elif output_kind == "fifo":
                os.mkfifo(target)
            else:
                raise AssertionError(output_kind)
            return record, b"", b""

        with mock.patch.object(
            benchmark, "run_build_capture", side_effect=fake_capture
        ):
            with self.assertRaises(benchmark.WorkloadBuildError) as captured:
                benchmark.build_reference_workload(
                    self.manifest["workloads"][0],
                    self.suite_root,
                    bundle,
                    toolchains,
                    120.0,
                    include_nomo_baseline=True,
                )
        failure = captured.exception.record
        self.assertEqual(failure["failure_kind"], "invalid-output")
        expected_kind = {
            "directory": ("directory", "directory"),
            "symlink": ("symlink", "symlink"),
            "fifo": ("other-nonregular", "fifo"),
        }[output_kind]
        self.assertEqual(
            (
                failure["output_state"]["kind"],
                failure["output_state"]["lstat_type"],
            ),
            expected_kind,
        )
        self.assertEqual(failure["command"]["exit_code"], 0)

        collector = mock.Mock()
        host_os = platform.system()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host(host_os)
        )
        host = {"os": host_os, "architecture": platform.machine()}
        unavailable = {
            "status": "unavailable",
            "reason": "fixture release capability unavailable",
            "emit_c_fallback_used": False,
        }
        with mock.patch.object(
            benchmark, "release_capability", return_value=unavailable
        ), mock.patch.object(
            benchmark,
            "build_reference_workload",
            side_effect=benchmark.WorkloadBuildError(
                str(captured.exception), failure
            ),
        ):
            result = benchmark.run_correctness(
                Namespace(),
                self.manifest,
                self.manifest_path,
                self.suite_root,
                output,
                {},
                toolchains,
                collector,
                host,
            )
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(result, self.manifest)
        v1.write_result(output, result)
        log_path = benchmark.write_evidence_log(output, result)
        reloaded = v1.read_json(output)
        benchmark.validate_result_schema(reloaded, self.result_schema_path)
        benchmark.validate_result(reloaded, self.manifest)
        self.assertIn("not a regular file", log_path.read_text())
        with mock.patch.object(
            benchmark, "run_suite", return_value=(output, result)
        ):
            self.assertEqual(benchmark.main(["--mode", "correctness"]), 2)

    def test_directory_build_output_is_retained_as_invalid_evidence(self) -> None:
        self.assert_invalid_output_evidence("directory")

    @unittest.skipIf(os.name == "nt", "symlink creation is not portable on Windows")
    def test_symlink_build_output_is_retained_as_invalid_evidence(self) -> None:
        self.assert_invalid_output_evidence("symlink")

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO creation requires POSIX")
    def test_fifo_build_output_is_retained_as_invalid_evidence(self) -> None:
        self.assert_invalid_output_evidence("fifo")

    def test_reference_builds_reject_stale_c_cpp_go_and_semantic_outputs(
        self,
    ) -> None:
        workload = self.manifest["workloads"][0]
        workload_id = workload["id"]
        toolchains = self.correctness_only_result()["provenance"]["toolchains"]
        for stale_lane in benchmark.REFERENCE_LANES:
            with self.subTest(stale_lane=stale_lane):
                bundle = Path(self.temporary.name) / f"stale-{stale_lane}"
                stale_output = benchmark.binary_path(
                    bundle / "bin", f"{workload_id}-{stale_lane}"
                )
                stale_output.parent.mkdir(parents=True)
                stale_output.write_bytes(b"stale-binary")
                executed: list[list[str]] = []

                def fake_capture(
                    command: list[str],
                    timeout_seconds: float,
                    cwd: Path | None = None,
                    approved_environment_overrides: dict[str, str]
                    | None = None,
                ) -> tuple[dict, bytes, bytes]:
                    del timeout_seconds
                    argv = [str(part) for part in command]
                    executed.append(argv)
                    output = Path(argv[argv.index("-o") + 1])
                    output.parent.mkdir(parents=True, exist_ok=True)
                    output.write_bytes(b"new-binary")
                    return (
                        self.full_command(
                            argv,
                            cwd=str(cwd.resolve()) if cwd else None,
                            approved_environment_overrides=(
                                approved_environment_overrides
                            ),
                        ),
                        b"",
                        b"",
                    )

                with mock.patch.object(
                    benchmark, "run_build_capture", side_effect=fake_capture
                ):
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, "refusing to reuse stale"
                    ):
                        benchmark.build_reference_workload(
                            workload,
                            self.suite_root,
                            bundle,
                            toolchains,
                            120.0,
                            include_nomo_baseline=False,
                        )
                self.assertTrue(stale_output.is_file())
                self.assertEqual(stale_output.read_bytes(), b"stale-binary")
                stale_argv = [
                    command
                    for command in executed
                    if "-o" in command
                    and Path(command[command.index("-o") + 1])
                    == stale_output
                ]
                self.assertEqual(stale_argv, [])

    def test_nomo_baseline_rejects_stale_project_generated_c_and_binary(
        self,
    ) -> None:
        workload = self.manifest["workloads"][0]
        workload_id = workload["id"]
        toolchains = self.correctness_only_result()["provenance"]["toolchains"]

        bundle = Path(self.temporary.name) / "stale-project"
        project = (
            bundle
            / "build"
            / workload_id
            / "references"
            / "nomo-baseline-project"
        )
        project.mkdir(parents=True)
        with mock.patch.object(
            benchmark, "run_build_capture"
        ) as build_capture:
            with self.assertRaisesRegex(
                benchmark.HarnessError, "reference build root already exists"
            ):
                benchmark.build_reference_workload(
                    workload,
                    self.suite_root,
                    bundle,
                    toolchains,
                    120.0,
                    include_nomo_baseline=True,
                )
        build_capture.assert_not_called()

        original_copy = v1.copy_nomo_project
        for stale_kind in ("generated-c", "binary"):
            with self.subTest(stale_kind=stale_kind):
                bundle = (
                    Path(self.temporary.name) / f"stale-baseline-{stale_kind}"
                )

                def copy_with_stale(
                    source: Path,
                    manifest: Path,
                    destination: Path,
                ) -> None:
                    original_copy(source, manifest, destination)
                    stale = (
                        destination / "build" / "c" / "main.c"
                        if stale_kind == "generated-c"
                        else benchmark.binary_path(
                            bundle / "bin",
                            f"{workload_id}-nomo-baseline",
                        )
                    )
                    stale.parent.mkdir(parents=True, exist_ok=True)
                    stale.write_bytes(b"stale-nomo-output")

                def fake_capture(
                    command: list[str],
                    timeout_seconds: float,
                    cwd: Path | None = None,
                    approved_environment_overrides: dict[str, str]
                    | None = None,
                ) -> tuple[dict, bytes, bytes]:
                    del timeout_seconds
                    argv = [str(part) for part in command]
                    if "-o" in argv:
                        output = Path(argv[argv.index("-o") + 1])
                        output.parent.mkdir(parents=True, exist_ok=True)
                        output.write_bytes(b"new-reference-output")
                    elif argv[-1] == "--emit-c":
                        generated = Path(argv[2]) / "build" / "c" / "main.c"
                        generated.parent.mkdir(parents=True, exist_ok=True)
                        generated.write_bytes(b"new-generated-c")
                    return (
                        self.full_command(
                            argv,
                            cwd=str(cwd.resolve()) if cwd else None,
                            approved_environment_overrides=(
                                approved_environment_overrides
                            ),
                        ),
                        b"",
                        b"",
                    )

                with mock.patch.object(
                    v1, "copy_nomo_project", side_effect=copy_with_stale
                ), mock.patch.object(
                    benchmark, "run_build_capture", side_effect=fake_capture
                ):
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, "refusing to reuse stale"
                    ):
                        benchmark.build_reference_workload(
                            workload,
                            self.suite_root,
                            bundle,
                            toolchains,
                            120.0,
                            include_nomo_baseline=True,
                        )

    def test_formal_release_and_emit_c_reject_stale_projects_and_outputs(
        self,
    ) -> None:
        workload = self.manifest["workloads"][0]
        workload_id = workload["id"]
        toolchains = self.completed_result()["provenance"]["toolchains"]
        lane_state = {
            "status": "available",
            "capabilities": {
                "release": {"status": "available"},
                "emit-c": {"status": "available"},
            },
            "nomo_path": toolchains["nomo"]["path"],
            "nomo_sha256": toolchains["nomo"].get("sha256", "0" * 64),
            "checkout": str(REPOSITORY_ROOT),
            "expected_commit": "a" * 40,
            "repository": {"commit": "a" * 40},
        }
        for build_mode, builder in (
            ("release", benchmark.build_release_lane),
            ("emit-c", benchmark.build_emit_c_lane),
        ):
            with self.subTest(build_mode=build_mode, stale="project"):
                bundle = (
                    Path(self.temporary.name) / f"formal-{build_mode}-project"
                )
                project = (
                    bundle
                    / "build"
                    / workload_id
                    / build_mode
                    / "candidate"
                    / "project"
                )
                project.mkdir(parents=True)
                with mock.patch.object(
                    benchmark, "run_build_capture"
                ) as build_capture:
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, "project already exists"
                    ):
                        builder(
                            workload,
                            self.suite_root,
                            bundle,
                            "candidate",
                            lane_state,
                            toolchains,
                            120.0,
                        )
                build_capture.assert_not_called()

        original_copy = v1.copy_nomo_project
        cases = (
            ("release", "binary", benchmark.build_release_lane),
            ("release", "generated-c", benchmark.build_release_lane),
            ("release", "provenance", benchmark.build_release_lane),
            ("release", "metadata", benchmark.build_release_lane),
            ("emit-c", "generated-c", benchmark.build_emit_c_lane),
            ("emit-c", "binary", benchmark.build_emit_c_lane),
        )
        for build_mode, stale_kind, builder in cases:
            with self.subTest(build_mode=build_mode, stale_kind=stale_kind):
                bundle = (
                    Path(self.temporary.name)
                    / f"formal-{build_mode}-{stale_kind}"
                )

                def copy_with_stale(
                    source: Path,
                    manifest: Path,
                    destination: Path,
                ) -> None:
                    original_copy(source, manifest, destination)
                    project_name = benchmark.parse_project_name(manifest)
                    stale_paths = {
                        "binary": benchmark.binary_path(
                            destination / "build" / "bin", project_name
                        ),
                        "generated-c": (
                            destination / "build" / "c" / "main.c"
                        ),
                        "provenance": (
                            destination
                            / "build"
                            / "release-provenance.json"
                        ),
                        "metadata": (
                            destination
                            / "build"
                            / "nomo-build-metadata.json"
                        ),
                    }
                    stale = stale_paths[stale_kind]
                    stale.parent.mkdir(parents=True, exist_ok=True)
                    stale.write_bytes(b"stale-formal-output")

                def fake_emit(
                    command: list[str],
                    timeout_seconds: float,
                    cwd: Path | None = None,
                    approved_environment_overrides: dict[str, str]
                    | None = None,
                ) -> tuple[dict, bytes, bytes]:
                    del timeout_seconds
                    argv = [str(part) for part in command]
                    if argv[-1] == "--emit-c":
                        generated = Path(argv[2]) / "build" / "c" / "main.c"
                        generated.parent.mkdir(parents=True, exist_ok=True)
                        generated.write_bytes(b"new-generated-c")
                    return (
                        self.full_command(
                            argv,
                            cwd=str(cwd.resolve()) if cwd else None,
                            approved_environment_overrides=(
                                approved_environment_overrides
                            ),
                        ),
                        b"",
                        b"",
                    )

                with mock.patch.object(
                    v1, "copy_nomo_project", side_effect=copy_with_stale
                ), mock.patch.object(
                    benchmark, "run_build_capture", side_effect=fake_emit
                ):
                    with self.assertRaisesRegex(
                        benchmark.HarnessError, "refusing to reuse stale"
                    ):
                        builder(
                            workload,
                            self.suite_root,
                            bundle,
                            "candidate",
                            lane_state,
                            toolchains,
                            120.0,
                        )

    def assert_formal_invalid_output_is_retained(
        self,
        stage: str,
        output_kind: str,
    ) -> None:
        root = (
            Path(self.temporary.name).resolve()
            / f"formal-invalid-{stage}-{output_kind}-{time.time_ns()}"
        )
        root.mkdir()
        bundle = root / "bundle"
        output = root / "prepare-result.json"
        fixture = self.project_result_to_producer_os(
            self.completed_result(), platform.system()
        )
        toolchains = fixture["provenance"]["toolchains"]
        self.bind_live_execution_authority(toolchains)
        host = {
            **fixture["provenance"]["host"],
            "os": platform.system(),
            "architecture": platform.machine(),
        }
        repository = fixture["provenance"]["repository"]
        release_lanes = [
            copy.deepcopy(fixture["release_lanes"][lane])
            for lane in ("candidate", "main")
        ]
        for lane, state in zip(("candidate", "main"), release_lanes):
            compiler = benchmark.binary_path(
                bundle / "compiler-build" / lane / "release", "nomo"
            )
            compiler.parent.mkdir(parents=True, exist_ok=True)
            compiler.write_bytes(f"{lane}-compiler".encode())
            compiler_sha = v1.sha256_file(compiler)
            state["nomo_path"] = str(compiler)
            state["nomo_sha256"] = compiler_sha
            state["compiler_build"]["binary"] = {
                "path": str(compiler),
                "sha256": compiler_sha,
            }
            for capability in state["capabilities"].values():
                capability["nomo_path"] = str(compiler)
                capability["nomo_sha256"] = compiler_sha
                capability["help_command"] = self.full_command(
                    [str(compiler), "build", "--help"],
                    cwd=state["checkout"],
                )

        workload = self.manifest["workloads"][0]
        workload_id = workload["id"]
        reference = fixture["builds"][workload_id]["references"]
        reference_binaries = {
            lane: Path(record["path"])
            for lane, record in reference["binaries"].items()
            if lane in benchmark.REFERENCE_LANES
        }

        def materialize(path: Path, kind: str) -> None:
            if kind == "absent":
                return
            path.parent.mkdir(parents=True, exist_ok=True)
            if kind == "regular":
                path.write_bytes(b"fixture-build-output")
            elif kind == "directory":
                path.mkdir()
            elif kind == "symlink":
                target = path.parent / f"{path.name}-symlink-target"
                target.write_bytes(b"not-the-build-output")
                path.symlink_to(target)
            elif kind == "fifo":
                os.mkfifo(path)
            else:
                raise AssertionError(kind)

        def fake_capture(
            command: list[str],
            timeout_seconds: float,
            cwd: Path | None = None,
            approved_environment_overrides: dict[str, str] | None = None,
        ) -> tuple[dict, bytes, bytes]:
            del timeout_seconds
            argv = [str(part) for part in command]
            record = self.full_command(
                argv,
                cwd=str(cwd.resolve()) if cwd is not None else None,
                approved_environment_overrides=approved_environment_overrides,
            )
            if argv[-1] == "--release":
                project = Path(argv[2])
                project_name = benchmark.parse_project_name(
                    self.suite_root
                    / workload["sources"]["nomo"]["project_manifest"]
                )
                outputs = {
                    "release-binary": benchmark.binary_path(
                        project / "build" / "bin", project_name
                    ),
                    "release-generated-c": (
                        project / "build" / "c" / "main.c"
                    ),
                    "release-provenance": (
                        project / "build" / "release-provenance.json"
                    ),
                    "release-metadata": (
                        project / "build" / "nomo-build-metadata.json"
                    ),
                }
                for output_stage, target in outputs.items():
                    materialize(
                        target,
                        output_kind if stage == output_stage else "regular",
                    )
            elif argv[-1] == "--emit-c":
                generated = Path(argv[2]) / "build" / "c" / "main.c"
                materialize(
                    generated,
                    output_kind
                    if stage == "emit-generated-c"
                    else "regular",
                )
            elif "-o" in argv:
                target = Path(argv[argv.index("-o") + 1])
                materialize(
                    target,
                    output_kind if stage == "emit-binary" else "regular",
                )
            return record, b"", b""

        arguments = Namespace(
            candidate_commit=release_lanes[0]["expected_commit"],
            main_commit=release_lanes[1]["expected_commit"],
            candidate_checkout=release_lanes[0]["checkout"],
            main_checkout=release_lanes[1]["checkout"],
            cargo="cargo",
            environment_qualification=None,
            prepared_bundle=str(bundle),
        )
        collector = mock.Mock()
        collector.descriptor.return_value = (
            benchmark.collector_descriptor_for_host(host["os"])
        )
        release_builds = {
            lane: (
                copy.deepcopy(
                    fixture["builds"][workload_id]["modes"]["release"][lane]
                ),
                Path(
                    fixture["builds"][workload_id]["modes"]["release"][lane][
                        "binary"
                    ]["path"]
                ),
            )
            for lane in ("candidate", "main")
        }

        patches = [
            mock.patch.object(
                benchmark,
                "release_lane_state",
                side_effect=release_lanes,
            ),
            mock.patch.object(
                benchmark,
                "build_reference_workload",
                return_value=(reference, reference_binaries),
            ),
            mock.patch.object(
                benchmark,
                "run_build_capture",
                side_effect=fake_capture,
            ),
        ]
        if stage.startswith("emit-"):
            patches.append(
                mock.patch.object(
                    benchmark,
                    "build_release_lane",
                    side_effect=[
                        release_builds["candidate"],
                        release_builds["main"],
                    ],
                )
            )
        with contextlib.ExitStack() as stack:
            for patch in patches:
                stack.enter_context(patch)
            result = benchmark.run_prepare(
                arguments,
                self.manifest,
                self.manifest_path,
                self.suite_root,
                output,
                repository,
                toolchains,
                collector,
                host,
            )
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(len(result["build_failures"]), 1)
        failure = result["build_failures"][0]
        self.assertEqual(failure["lane"], "candidate")
        expected_phase = {
            "release-binary": "nomo-release-binary",
            "release-generated-c": "nomo-release-generated-c",
            "release-provenance": "nomo-release-provenance",
            "release-metadata": "nomo-release-metadata",
            "emit-generated-c": "nomo-emit-c",
            "emit-binary": "nomo-generated-c-clang",
        }[stage]
        self.assertEqual(failure["phase"], expected_phase)
        self.assertEqual(
            failure["failure_kind"],
            "missing-output" if output_kind == "absent" else "invalid-output",
        )
        benchmark.validate_result_schema(result, self.result_schema_path)
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ):
            benchmark.validate_result(result, self.manifest)
        v1.write_result(output, result)
        log_path = benchmark.write_evidence_log(output, result)
        reloaded = v1.read_json(output)
        benchmark.validate_result_schema(reloaded, self.result_schema_path)
        with mock.patch.object(
            benchmark, "validate_release_lane_authority"
        ):
            benchmark.validate_result(reloaded, self.manifest)
        self.assertIn(expected_phase, log_path.read_text(encoding="utf-8"))
        with mock.patch.object(
            benchmark, "run_suite", return_value=(output, result)
        ):
            self.assertEqual(benchmark.main(["--mode", "prepare"]), 2)

    def test_formal_prepare_retains_each_invalid_output_phase(self) -> None:
        for stage in (
            "release-binary",
            "release-generated-c",
            "release-provenance",
            "release-metadata",
            "emit-generated-c",
            "emit-binary",
        ):
            with self.subTest(stage=stage):
                self.assert_formal_invalid_output_is_retained(
                    stage, "directory"
                )

    def test_formal_failure_uses_recorded_host_math_contract(
        self,
    ) -> None:
        self.assert_formal_invalid_output_is_retained(
            "emit-binary", "directory"
        )

    def test_formal_prepare_retains_absent_symlink_and_nonregular_outputs(
        self,
    ) -> None:
        kinds = ["absent", "directory"]
        symlink_probe = (
            Path(self.temporary.name).resolve() / "formal-symlink-probe"
        )
        symlink_target = symlink_probe.with_name("formal-symlink-target")
        try:
            symlink_probe.symlink_to(symlink_target)
        except OSError:
            pass
        else:
            symlink_probe.unlink()
            kinds.append("symlink")
        if hasattr(os, "mkfifo"):
            kinds.append("fifo")
        for stage in ("release-binary", "release-metadata"):
            for output_kind in kinds:
                with self.subTest(stage=stage, output_kind=output_kind):
                    self.assert_formal_invalid_output_is_retained(
                        stage, output_kind
                    )

    def test_clang_driver_config_is_required_once_on_every_decisive_path(
        self,
    ) -> None:
        clang = shutil.which("clang")
        if clang is None:
            self.skipTest("Clang is unavailable")
        subprocess.run(
            [clang, *benchmark.CLANG_DRIVER_CONFIG_FLAGS, "--version"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        result = self.completed_result()
        commands = [
            result["builds"]["spectral-norm"]["references"]["commands"][
                "c_build"
            ],
            result["builds"]["spectral-norm"]["modes"]["emit-c"][
                "candidate"
            ]["clang_command"],
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance"]["compile_commands"][0],
            result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance"]["link_command"],
        ]
        for command in commands:
            self.assertEqual(
                command["argv"].count("--no-default-config"), 1
            )

        missing = copy.deepcopy(result)
        command = missing["builds"]["spectral-norm"]["modes"]["emit-c"][
            "candidate"
        ]["clang_command"]
        command["argv"].remove("--no-default-config")
        command["command"] = v1.command_text(command["argv"])
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Clang argv changed"
        ):
            benchmark.validate_build_provenance(missing, self.manifest)

        duplicated = copy.deepcopy(result)
        command = duplicated["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]["backend_provenance"]["compile_commands"][0]
        command["argv"].insert(1, "--no-default-config")
        command["command"] = v1.command_text(command["argv"])
        with self.assertRaisesRegex(
            benchmark.HarnessError, "backend C argv changed"
        ):
            benchmark.validate_build_provenance(duplicated, self.manifest)

        with self.assertRaisesRegex(
            benchmark.HarnessError, "external Clang driver config"
        ):
            benchmark.validate_clang_driver_config_argv(
                [
                    clang,
                    "--no-default-config",
                    f"--config={Path(self.temporary.name) / 'poison.cfg'}",
                ],
                "poisoned Clang",
            )

    def test_controlled_build_path_ignores_temporary_arg0_entries(self) -> None:
        temporary_a = (
            Path(self.temporary.name) / ".codex" / "tmp" / "arg0" / "codex-arg0-a"
        )
        temporary_b = (
            Path(self.temporary.name) / ".codex" / "tmp" / "arg0" / "codex-arg0-b"
        )
        injected = Path(self.temporary.name) / "injected-tool-directory"
        temporary_a.mkdir(parents=True)
        temporary_b.mkdir(parents=True)
        injected.mkdir()
        stable = benchmark.stable_build_path()
        with mock.patch.dict(
            os.environ,
            {
                "PATH": os.pathsep.join(
                    (str(temporary_a), str(injected), stable)
                )
            },
        ):
            first = benchmark.sanitized_build_environment()[1]
        with mock.patch.dict(
            os.environ,
            {
                "PATH": os.pathsep.join(
                    (str(temporary_b), str(injected), stable)
                )
            },
        ):
            second = benchmark.sanitized_build_environment()[1]
        self.assertEqual(first, second)
        self.assertNotIn(str(temporary_a), first["retained"]["PATH"])
        self.assertNotIn(str(injected), first["retained"]["PATH"])

    def test_prepare_measure_arg0_paths_have_same_cross_process_identity(
        self,
    ) -> None:
        arg0_a = (
            Path(self.temporary.name) / ".codex" / "tmp" / "arg0" / "prepare"
        )
        arg0_b = (
            Path(self.temporary.name) / ".codex" / "tmp" / "arg0" / "measure"
        )
        arg0_a.mkdir(parents=True)
        arg0_b.mkdir(parents=True)
        module_path = Path(benchmark.__file__).resolve()
        script = """
import json
import sys
sys.path.insert(0, str(__import__("pathlib").Path(sys.argv[1]).parent))
import benchmarksgame_v2 as benchmark
toolchains = {
    name: {
        "path": f"/stable/{name}",
        "realpath": f"/stable/{name}",
        "sha256": "0" * 64,
        "version": "1.0.0",
        "version_output": "stable",
        "installation": "/stable",
        "target_triple": "stable-target",
    }
    for name in ("nomo", "clang", "clangxx", "go")
}
print(json.dumps(
    benchmark.stable_toolchain_identity(toolchains),
    sort_keys=True,
))
"""

        def identity(arg0: Path) -> str:
            environment = os.environ.copy()
            environment["PATH"] = os.pathsep.join(
                (str(arg0), benchmark.stable_build_path())
            )
            completed = subprocess.run(
                [sys.executable, "-c", script, str(module_path)],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            return completed.stdout

        self.assertEqual(identity(arg0_a), identity(arg0_b))
        self.assertNotIn(".codex/tmp/arg0", identity(arg0_a))

    def test_formal_authority_checkout_is_always_required_clean(self) -> None:
        root = Path(self.temporary.name).resolve() / "clean-authority"
        arguments = benchmark.parse_arguments(
            [
                "--mode",
                "prepare",
                "--output",
                str(root / "result.json"),
                "--prepared-bundle",
                str(root / "bundle"),
            ]
        )
        with mock.patch.object(
            v1,
            "repository_state",
            side_effect=benchmark.HarnessError("dirty checkout"),
        ) as repository_state:
            with self.assertRaisesRegex(benchmark.HarnessError, "dirty checkout"):
                benchmark.run_suite(arguments)
        self.assertTrue(repository_state.call_args.kwargs["require_clean"])

    def test_formal_measurement_rejects_external_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            external = Path(temporary) / "manifest-v2.json"
            external.write_text(self.manifest_path.read_text(), encoding="utf-8")
            arguments = benchmark.parse_arguments(
                ["--mode", "measure", "--manifest", str(external)]
            )
            with self.assertRaisesRegex(benchmark.HarnessError, "canonical checked-in"):
                benchmark.run_suite(arguments)

    def test_dual_build_provenance_rejects_release_fallback_and_emit_c_drift(
        self,
    ) -> None:
        result = self.completed_result()
        benchmark.validate_build_provenance(result, self.manifest)
        changed = copy.deepcopy(result)
        command = changed["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]["command"]
        command["argv"] = [
            self.fixture_path("tools", "nomo"),
            "build",
            self.fixture_path("project"),
            "--emit-c",
        ]
        command["command"] = v1.command_text(command["argv"])
        with self.assertRaisesRegex(benchmark.HarnessError, "release argv changed"):
            benchmark.validate_build_provenance(changed, self.manifest)
        changed = copy.deepcopy(result)
        clang = changed["builds"]["spectral-norm"]["modes"]["emit-c"]["candidate"][
            "clang_command"
        ]
        clang["argv"].remove("-O3")
        clang["command"] = v1.command_text(clang["argv"])
        with self.assertRaisesRegex(benchmark.HarnessError, "Clang argv changed"):
            benchmark.validate_build_provenance(changed, self.manifest)

    def test_formal_slots_exactly_bind_lane_repository_and_nomo_identity(
        self,
    ) -> None:
        result = self.completed_result()
        for build_mode in benchmark.FORMAL_BUILD_MODES:
            for mutation in ("lane", "repository", "nomo-path"):
                changed = copy.deepcopy(result)
                formal = changed["builds"]["spectral-norm"]["modes"][
                    build_mode
                ]["candidate"]
                if mutation == "lane":
                    formal["lane"] = "main"
                elif mutation == "repository":
                    formal["repository"]["dirty"] = True
                else:
                    formal["nomo"]["path"] = "/outside/same-bytes-nomo"
                with self.subTest(
                    build_mode=build_mode, mutation=mutation
                ), self.assertRaisesRegex(
                    benchmark.HarnessError, "formal slot"
                ):
                    benchmark.validate_build_provenance(
                        changed, self.manifest
                    )

    def test_formal_generated_c_markers_are_mode_specific_and_required(
        self,
    ) -> None:
        result = self.completed_result()
        cases = (
            ("release", "unmodified_after_build", "unmodified_after_emit"),
            ("emit-c", "unmodified_after_emit", "unmodified_after_build"),
        )
        for build_mode, required_marker, wrong_marker in cases:
            for mutation in ("missing", "wrong-mode"):
                changed = copy.deepcopy(result)
                generated = changed["builds"]["spectral-norm"]["modes"][
                    build_mode
                ]["candidate"]["generated_c"]
                generated.pop(required_marker)
                if mutation == "wrong-mode":
                    generated[wrong_marker] = True
                with self.subTest(
                    build_mode=build_mode, mutation=mutation
                ):
                    with self.assertRaises(benchmark.HarnessError):
                        benchmark.validate_result_schema(
                            changed, self.result_schema_path
                        )
                    with self.assertRaisesRegex(
                        benchmark.HarnessError,
                        "outputs are not|project-bound",
                    ):
                        benchmark.validate_build_provenance(
                            changed, self.manifest
                        )

    def test_reference_provenance_rejects_cwd_output_and_hidden_sources(
        self,
    ) -> None:
        result = self.completed_result()
        for mutation in ("cwd", "compiler-output", "hidden-source"):
            changed = copy.deepcopy(result)
            references = changed["builds"]["spectral-norm"]["references"]
            if mutation == "cwd":
                references["commands"]["c_build"]["cwd"] = "/arbitrary"
            elif mutation == "compiler-output":
                references["compiler_output"] = {}
            else:
                references["source_files"]["hidden"] = {
                    "path": "/outside/hidden-input.c",
                    "sha256": "0" * 64,
                }
            with self.subTest(mutation=mutation), self.assertRaises(
                benchmark.HarnessError
            ):
                benchmark.validate_build_provenance(changed, self.manifest)

    def test_independent_emit_c_lane_builds_unmodified_generated_c(self) -> None:
        nomo = benchmark.binary_path(
            REPOSITORY_ROOT / "target" / "release", "nomo"
        )
        clang = shutil.which("clang")
        if not nomo.is_file() or clang is None:
            self.skipTest("release Nomo driver or Clang is unavailable")
        workload = self.manifest["workloads"][0]
        lane_state = {
            "status": "available",
            "checkout": str(REPOSITORY_ROOT),
            "expected_commit": "a" * 40,
            "repository": {"commit": "a" * 40},
            "nomo_path": str(nomo),
            "nomo_sha256": v1.sha256_file(nomo),
            "capabilities": {"emit-c": {"status": "available"}},
        }
        with tempfile.TemporaryDirectory() as temporary:
            build, binary = benchmark.build_emit_c_lane(
                workload,
                self.suite_root,
                Path(temporary),
                "candidate",
                lane_state,
                {"clang": {"path": clang}},
                120.0,
            )
            fixture = (
                self.suite_root / workload["fixtures"]["correctness"]["path"]
            ).read_bytes()
            completed = subprocess.run(
                [str(binary), workload["correctness_input"]],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        self.assertEqual(
            benchmark._validate_process_output(
                [str(binary), workload["correctness_input"]],
                completed.returncode,
                completed.stdout,
                completed.stderr,
                fixture,
            ),
            fixture,
        )
        self.assertTrue(build["generated_c"]["unmodified_after_emit"])
        self.assertFalse(build["release_artifact_reused"])
        self.assertEqual(
            tuple(build["clang_command"]["argv"][1:2]),
            benchmark.CLANG_DRIVER_CONFIG_FLAGS,
        )
        self.assertEqual(
            tuple(build["clang_command"]["argv"][2:6]),
            benchmark.BASE_C_FLAGS,
        )

    def test_draft_2020_12_schema_accepts_compiler_fields_only_nested(self) -> None:
        if benchmark.Draft202012Validator is None:
            self.fail("install scripts/requirements-benchmarksgame-v2.txt")
        result = self.correctness_only_result()
        result["release_lanes"] = {
            lane: self.available_release_lane(lane)
            for lane in ("candidate", "main")
        }
        benchmark.validate_result_schema(result, self.result_schema_path)
        changed = copy.deepcopy(result)
        changed["release_lanes"]["main"]["origin_main_commit"] = "b" * 40
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Draft 2020-12 schema"
        ):
            benchmark.validate_result_schema(changed, self.result_schema_path)

    def test_complete_dual_mode_artifact_validates_schema_and_recomputation(
        self,
    ) -> None:
        result = self.project_windows_paths_as_linux_artifact(
            self.completed_result()
        )
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(
            result, self.manifest, offline_replay=True
        )

    def test_correctness_only_projects_windows_paths_before_authorization(
        self,
    ) -> None:
        def windows_fixture_path(*parts: str) -> str:
            return str(
                benchmark.PureWindowsPath(
                    "D:/native-windows-producer", *parts
                )
            )

        with mock.patch.object(
            self,
            "fixture_path",
            side_effect=windows_fixture_path,
        ), mock.patch.object(
            self,
            "manifest_path",
            benchmark.PureWindowsPath(
                "D:/native-windows-producer/manifest-v2.json"
            ),
        ):
            result = self.correctness_only_result()
        provenance = result["provenance"]
        self.assertEqual(provenance["host"]["os"], "Linux")
        self.assertTrue(
            provenance["manifest_path"].startswith(
                "/recorded-windows-producer/d/"
            )
        )
        self.assertTrue(
            provenance["toolchains"]["nomo"]["path"].startswith(
                "/recorded-windows-producer/d/"
            )
        )
        expected_bindings = benchmark.qualification_bindings(
            provenance["host"],
            provenance["toolchains"],
            provenance["source_lock"],
            result["release_lanes"],
        )
        self.assertEqual(
            provenance["environment_qualification"][
                "expected_bindings"
            ],
            expected_bindings,
        )
        benchmark.validate_result_schema(result, self.result_schema_path)

    def test_windows_authorization_path_projects_to_recorded_linux(
        self,
    ) -> None:
        result = self.completed_result()
        result["provenance"]["environment_qualification"][
            "qualification_path"
        ] = (
            r"C:\Users\runneradmin\AppData\Local\Temp"
            r"\benchmark-authority\environment.json"
        )
        result = self.project_windows_paths_as_linux_artifact(result)
        qualification_path = result["provenance"][
            "environment_qualification"
        ]["qualification_path"]
        self.assertEqual(result["provenance"]["host"]["os"], "Linux")
        self.assertTrue(
            benchmark.PurePosixPath(qualification_path).is_absolute()
        )
        self.assertTrue(
            qualification_path.startswith(
                "/recorded-windows-producer/c/"
            )
        )
        benchmark.validate_result(
            result, self.manifest, offline_replay=True
        )

    def test_collector_descriptor_and_sample_derived_fields_are_authoritative(
        self,
    ) -> None:
        result = self.completed_result()
        for field, value in (
            ("wall_clock", "fabricated-clock"),
            ("id", "fabricated-collector"),
        ):
            changed = copy.deepcopy(result)
            changed["provenance"]["collector"][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(
                benchmark.HarnessError, "collector descriptor"
            ):
                benchmark.validate_result(changed, self.manifest)

        for field, value in (
            ("cpu_total_ns", 999),
            ("stdout_raw_sha256", "0" * 64),
            (
                "stdout_normalized_sha256",
                "0" * 64,
            ),
        ):
            changed = copy.deepcopy(result)
            sample = changed["protocols"]["release"]["batches"][0][
                "workloads"
            ][0]["samples"]["candidate"][0]
            sample[field] = value
            with self.subTest(field=field), mock.patch.object(
                benchmark, "validate_result_prepared_authority"
            ), self.assertRaises(benchmark.HarnessError):
                benchmark.validate_result(changed, self.manifest)

    def test_completed_rejects_empty_environment_and_fake_sample(self) -> None:
        result = self.completed_result()
        result["provenance"]["environment_qualification"] = {}
        with self.assertRaisesRegex(benchmark.HarnessError, "static authorization"):
            benchmark.validate_result(result, self.manifest)
        result = self.completed_result()
        sample = result["protocols"]["release"]["batches"][0]["workloads"][0][
            "samples"
        ]["candidate"][0]
        sample["command_argv"] = [str(Path(sys.executable).resolve())]
        sample["command"] = v1.command_text(sample["command_argv"])
        sample["executable_sha256"] = "0" * 64
        sample["stdout_raw_sha256"] = "0" * 64
        sample["stdout_normalized_sha256"] = "0" * 64
        with mock.patch.object(benchmark, "validate_result_prepared_authority"):
            with self.assertRaisesRegex(
                benchmark.HarnessError, "switched executable|fixture"
            ):
                benchmark.validate_result(result, self.manifest)

    def test_static_authorization_is_rederived_from_qualification_file(
        self,
    ) -> None:
        result = self.completed_result()
        embedded = result["provenance"]["environment_qualification"]
        embedded["checks"]["os_kernel"]["value"] = "self-consistent tamper"
        embedded["checks"]["os_kernel"]["evidence"][
            "value_sha256"
        ] = benchmark.canonical_json_sha256("self-consistent tamper")
        with mock.patch.object(benchmark, "validate_result_prepared_authority"):
            with self.assertRaisesRegex(
                benchmark.HarnessError,
                "differs from the qualification file",
            ):
                benchmark.validate_result(result, self.manifest)

    def test_completed_rejects_empty_correctness_and_release_lane_fields(self) -> None:
        result = self.completed_result()
        result["protocols"]["release"]["correctness"][0]["implementations"] = {}
        with mock.patch.object(benchmark, "validate_result_prepared_authority"):
            with self.assertRaisesRegex(
                benchmark.HarnessError, "correctness .*prefix|correctness contract"
            ):
                benchmark.validate_result(result, self.manifest)
        result = self.completed_result()
        result["release_lanes"]["candidate"].pop("compiler_build")
        with self.assertRaisesRegex(benchmark.HarnessError, "incomplete"):
            benchmark.validate_release_lane_authority(result)

    def test_top_level_status_cannot_downgrade_two_completed_protocols(self) -> None:
        result = self.completed_result()
        result["status"] = "ineligible"
        result["overall_verdict"] = "not_evaluated"
        result["claims"]["claim_eligible"] = False
        with self.assertRaisesRegex(
            benchmark.HarnessError, "does not match protocols"
        ):
            benchmark.validate_result(result, self.manifest)

    @unittest.skipUnless(hasattr(os, "wait4"), "requires POSIX wait4 collector")
    def test_formal_correctness_failures_are_retained_and_validate(
        self,
    ) -> None:
        root = Path(self.temporary.name)
        programs = {}
        mismatch = root / "correctness-mismatch"
        mismatch.write_text(
            f"#!{sys.executable}\nprint('wrong')\n",
            encoding="utf-8",
        )
        mismatch.chmod(0o755)
        programs["output-mismatch"] = mismatch
        timeout = root / "correctness-timeout"
        timeout.write_text(
            f"#!{sys.executable}\nimport time\ntime.sleep(10)\n",
            encoding="utf-8",
        )
        timeout.chmod(0o755)
        programs["timeout"] = timeout
        collector = benchmark.PosixWait4Collector()
        for failure_kind, executable in programs.items():
            with self.subTest(failure_kind=failure_kind):
                result = self.completed_result()
                result["provenance"]["collector"] = (
                    benchmark.collector_descriptor_for_host("Linux")
                )
                bound_binary = self.bind_formal_candidate_binary(
                    result, "spectral-norm", "release", executable
                )
                bound_executable = Path(bound_binary["path"])
                binaries = {
                    workload_id: {}
                    for workload_id in benchmark.WORKLOAD_IDS
                }
                binaries["spectral-norm"]["candidate"] = bound_executable
                collection_manifest = copy.deepcopy(self.manifest)
                if failure_kind == "timeout":
                    collection_manifest["methodology"][
                        "correctness_timeout_seconds"
                    ] = 0.02
                protocol = benchmark.protocol_result(
                    collection_manifest,
                    self.suite_root,
                    binaries,
                    collector,
                    "release",
                    result["provenance"]["environment_qualification"],
                )
                failed = protocol["correctness"][-1]["implementations"][
                    "candidate"
                ]["sample"]
                self.assertEqual(protocol["status"], "ineligible")
                self.assertEqual(protocol["batches"], [])
                self.assertEqual(failed["failure_kind"], failure_kind)
                result["protocols"]["release"] = protocol
                for correctness_item in result["protocols"]["emit-c"][
                    "correctness"
                ]:
                    for completed_implementation in correctness_item[
                        "implementations"
                    ].values():
                        completed_implementation["sample"][
                            "collector"
                        ] = collector.collector_id
                for batch in result["protocols"]["emit-c"]["batches"]:
                    for measured_workload in batch["workloads"]:
                        self.bind_collector(
                            measured_workload, collector.collector_id
                        )
                self.bind_synthetic_runtime_environments(result)
                aggregate = benchmark.aggregate_protocol_outcome(
                    result["protocols"]
                )
                result["status"] = aggregate["status"]
                result["overall_verdict"] = aggregate["overall_verdict"]
                result["claims"]["claim_eligible"] = aggregate[
                    "claim_eligible"
                ]
                benchmark.validate_result_schema(
                    result, self.result_schema_path
                )
                with mock.patch.object(
                    benchmark, "validate_result_prepared_authority"
                ):
                    benchmark.validate_result(result, self.manifest)

    def test_correctness_mode_retains_each_failed_workload_and_exits_nonzero(
        self,
    ) -> None:
        for failed_index, workload_id in enumerate(benchmark.WORKLOAD_IDS):
            with self.subTest(workload_id=workload_id):
                result = self.correctness_only_result()
                item = result["correctness"][failed_index]
                lane = benchmark.CORRECTNESS_BASELINE_LANES[0]
                implementation = item["implementations"][lane]
                sample = implementation["sample"]
                failure_message = f"{workload_id} fixture mismatch"
                mismatched_stdout = b"x"
                mismatched_sha256 = v1.sha256_bytes(mismatched_stdout)
                failed_sample = {
                    **sample,
                    "status": "failed",
                    "failure_kind": "output-mismatch",
                    "failure_message": failure_message,
                    "failure_source": "process-collector",
                    "stdout": "captured-failed",
                    "timed_out": False,
                    "stdout_raw_sha256": mismatched_sha256,
                    "stdout_normalized_sha256": mismatched_sha256,
                    "stdout_raw_base64": base64.b64encode(
                        mismatched_stdout
                    ).decode("ascii"),
                    "stdout_bytes": {"raw": 1, "normalized": 1},
                    "stderr": {
                        "sha256": v1.sha256_bytes(b""),
                        "length_bytes": 0,
                        "preview_utf8": "",
                    },
                }
                item["attempted_lanes"] = [lane]
                item["status"] = "failed"
                item["failure_reason"] = failure_message
                item["implementations"] = {
                    lane: {
                        "passed": False,
                        "stdout_normalized_sha256": mismatched_sha256,
                        "sample": failed_sample,
                    }
                }
                result["correctness"] = result["correctness"][
                    : failed_index + 1
                ]
                result["status"] = "ineligible"
                benchmark.validate_result_schema(
                    result, self.result_schema_path
                )
                benchmark.validate_result(result, self.manifest)
                output = (
                    Path(self.temporary.name)
                    / f"correctness-failed-{failed_index}.json"
                )
                v1.write_result(output, result)
                reloaded = v1.read_json(output)
                self.assertEqual(reloaded, result)
                benchmark.validate_result_schema(
                    reloaded, self.result_schema_path
                )
                benchmark.validate_result(reloaded, self.manifest)
                with mock.patch.object(
                    benchmark,
                    "run_suite",
                    return_value=(output, result),
                ):
                    self.assertEqual(benchmark.main([]), 2)

    def test_correctness_success_survives_sorted_json_round_trip(self) -> None:
        result = self.correctness_only_result()
        output = Path(self.temporary.name) / "correctness-success.json"
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_result(result, self.manifest)
        v1.write_result(output, result)
        reloaded = v1.read_json(output)
        benchmark.validate_result_schema(reloaded, self.result_schema_path)
        benchmark.validate_result(reloaded, self.manifest)

    @unittest.skipUnless(hasattr(os, "wait4"), "requires POSIX wait4 collector")
    def test_real_failures_preserve_second_and_third_workload_prefixes(
        self,
    ) -> None:
        timeout_program = Path("/bin/sleep")
        mismatch_program = Path("/usr/bin/printf")
        if not timeout_program.is_file() or not mismatch_program.is_file():
            self.skipTest("requires deterministic POSIX sleep and printf")
        result = self.completed_result()
        collector = benchmark.PosixWait4Collector()
        result["provenance"]["collector"] = (
            benchmark.collector_descriptor_for_host("Linux")
        )
        result["status"] = "ineligible"
        result["overall_verdict"] = "not_evaluated"
        result["claims"]["claim_eligible"] = False
        result["correctness"] = []
        authorization = result["provenance"]["environment_qualification"]
        snapshot_factory = self.dynamic_snapshot_factory()
        configurations = {
            "release": (
                "n-body",
                timeout_program,
                "timeout",
                ("spectral-norm",),
            ),
            "emit-c": (
                "fannkuch-redux",
                mismatch_program,
                "output-mismatch",
                ("spectral-norm", "n-body"),
            ),
        }
        for build_mode, (
            failed_workload,
            executable,
            failure_kind,
            prefix_ids,
        ) in configurations.items():
            bound_binary = self.bind_formal_candidate_binary(
                result, failed_workload, build_mode, executable
            )
            executable = Path(bound_binary["path"])
            batches = []
            for attempt_index in (1, 2):
                failed = self.real_failed_partial_workload(
                    result,
                    failed_workload,
                    build_mode,
                    attempt_index,
                    executable,
                    collector,
                    failure_kind,
                )
                failed_sample = failed["warmups"]["candidate"][0]
                failed_started = benchmark.parse_utc_timestamp(
                    failed_sample["started_at_utc"]
                )
                failed_finished = benchmark.parse_utc_timestamp(
                    failed_sample["finished_at_utc"]
                )
                prefix = [
                    self.measured_workload(
                        workload_id, build_mode, 1, attempt_index
                    )
                    for workload_id in prefix_ids
                ]
                for workload in prefix:
                    self.bind_collector(workload, collector.collector_id)
                    for phase in ("warmups", "samples"):
                        for lane_samples in workload[phase].values():
                            for sample in lane_samples:
                                sample["started_at_utc"] = (
                                    failed_started - dt.timedelta(seconds=2)
                                ).isoformat()
                                sample["finished_at_utc"] = (
                                    failed_started - dt.timedelta(seconds=1)
                                ).isoformat()
                before = snapshot_factory(
                    authorization["expected_bindings"][
                        "authority_host_sha256"
                    ]
                )
                before["captured_at_utc"] = (
                    failed_started - dt.timedelta(seconds=3)
                ).isoformat()
                before_body = {
                    key: value
                    for key, value in before.items()
                    if key != "snapshot_sha256"
                }
                before["snapshot_sha256"] = (
                    benchmark.canonical_json_sha256(before_body)
                )
                after = snapshot_factory(
                    authorization["expected_bindings"][
                        "authority_host_sha256"
                    ]
                )
                after["captured_at_utc"] = (
                    failed_finished + dt.timedelta(seconds=1)
                ).isoformat()
                after_body = {
                    key: value
                    for key, value in after.items()
                    if key != "snapshot_sha256"
                }
                after["snapshot_sha256"] = (
                    benchmark.canonical_json_sha256(after_body)
                )
                batches.append(
                    {
                        "build_mode": build_mode,
                        "batch_index": 1,
                        "attempt_index": attempt_index,
                        "started_at_utc": (
                            failed_started - dt.timedelta(seconds=4)
                        ).isoformat(),
                        "finished_at_utc": (
                            failed_finished + dt.timedelta(seconds=2)
                        ).isoformat(),
                        "static_authorization_sha256": authorization[
                            "qualification_sha256"
                        ],
                        "dynamic_environment_before": before,
                        "dynamic_environment_after": after,
                        "workloads": [*prefix, failed],
                        "status": "invalidated",
                        "invalidation_reasons": [
                            failed["failure_reason"]
                        ],
                        "stability": None,
                        "evaluation": {
                            "environment_eligible": False,
                            "verdict": "ineligible",
                        },
                    }
                )
            result["protocols"][build_mode] = {
                "build_mode": build_mode,
                "status": "ineligible",
                "reason": "structured failed attempt and retry retained",
                "correctness": [],
                "batches": batches,
                "verdict": "not_evaluated",
            }
        self.bind_synthetic_runtime_environments(result)
        benchmark.validate_result_schema(result, self.result_schema_path)
        with mock.patch.object(benchmark, "validate_result_prepared_authority"):
            benchmark.validate_result(result, self.manifest)
        self.assertEqual(
            [
                len(
                    result["protocols"][mode]["batches"][0][
                        "workloads"
                    ]
                )
                for mode in benchmark.FORMAL_BUILD_MODES
            ],
            [2, 3],
        )

    def test_unavailable_result_cannot_claim_or_pass(self) -> None:
        result = self.correctness_only_result()
        result["mode"] = "measure"
        result["status"] = "unavailable"
        result["correctness"] = []
        benchmark.validate_result(result, self.manifest)
        result["claims"]["claim_eligible"] = True
        with self.assertRaisesRegex(benchmark.HarnessError, "cannot be claim-eligible"):
            benchmark.validate_result(result, self.manifest)

    def test_compile_command_provenance_pins_complete_flags(self) -> None:
        benchmark.validate_build_provenance(
            self.completed_result(),
            self.manifest,
        )
        result = self.completed_result()
        changed = result["builds"]["spectral-norm"]["references"]
        changed["commands"]["cpp_build"]["argv"].remove("-pedantic-errors")
        changed["commands"]["cpp_build"]["command"] = v1.command_text(
            changed["commands"]["cpp_build"]["argv"]
        )
        with self.assertRaisesRegex(benchmark.HarnessError, "complete frozen argv"):
            benchmark.validate_build_provenance(
                result,
                self.manifest,
            )

    def test_nomo_baseline_build_requires_exact_six_commands(self) -> None:
        result = self.correctness_only_result()
        benchmark.validate_result_schema(result, self.result_schema_path)
        benchmark.validate_build_provenance(result, self.manifest)

        for missing_name in benchmark.NOMO_BASELINE_BUILD_COMMANDS:
            changed = copy.deepcopy(result)
            del changed["builds"]["spectral-norm"]["references"]["commands"][
                missing_name
            ]
            with self.assertRaisesRegex(
                benchmark.HarnessError, "Draft 2020-12 schema"
            ):
                benchmark.validate_result_schema(
                    changed, self.result_schema_path
                )
            with self.assertRaisesRegex(
                benchmark.HarnessError, "exact frozen command set"
            ):
                benchmark.validate_build_provenance(
                    changed, self.manifest
                )

        changed = copy.deepcopy(result)
        reference = changed["builds"]["spectral-norm"]["references"]
        reference["commands"]["unrecorded_extra_build"] = self.full_command(
            [
                self.fixture_path("tools", "clang"),
                "--fast-math",
                "--hidden-build",
            ]
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Draft 2020-12 schema"
        ):
            benchmark.validate_result_schema(
                changed, self.result_schema_path
            )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "exact frozen command set"
        ):
            benchmark.validate_build_provenance(changed, self.manifest)

    def test_nomo_baseline_emit_and_clang_argv_are_fully_pinned(self) -> None:
        result = self.correctness_only_result()
        changed = copy.deepcopy(result)
        emit = changed["builds"]["spectral-norm"]["references"]["commands"][
            "nomo_baseline_emit_c"
        ]
        emit["argv"] = [
            self.fixture_path("other-nomo"),
            "build",
            self.fixture_path("other-project"),
            "--release",
            "--emit-c",
        ]
        emit["command"] = v1.command_text(emit["argv"])
        with self.assertRaisesRegex(
            benchmark.HarnessError, "baseline emit-C argv changed"
        ):
            benchmark.validate_build_provenance(changed, self.manifest)

        changed = copy.deepcopy(result)
        clang = changed["builds"]["spectral-norm"]["references"]["commands"][
            "nomo_baseline_clang"
        ]
        clang["argv"].remove("-O3")
        clang["command"] = v1.command_text(clang["argv"])
        with self.assertRaisesRegex(
            benchmark.HarnessError, "baseline generated-C argv changed"
        ):
            benchmark.validate_build_provenance(changed, self.manifest)

        for tool in ("nomo", "clang"):
            changed = copy.deepcopy(result)
            changed["provenance"]["toolchains"][tool]["sha256"] = "invalid"
            with self.assertRaisesRegex(
                benchmark.HarnessError, "tool identity is incomplete"
            ):
                benchmark.validate_build_provenance(
                    changed, self.manifest
                )

    def test_reference_only_build_rejects_extra_command(self) -> None:
        result = self.completed_result()
        reference = result["builds"]["spectral-norm"]["references"]
        reference["commands"]["unrecorded_extra_build"] = self.full_command(
            [
                self.fixture_path("tools", "clang"),
                "--fast-math",
                "--hidden-build",
            ]
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "Draft 2020-12 schema"
        ):
            benchmark.validate_result_schema(
                result, self.result_schema_path
            )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "exact frozen command set"
        ):
            benchmark.validate_build_provenance(result, self.manifest)

    def test_formal_project_source_commit_and_cwd_are_exactly_bound(
        self,
    ) -> None:
        mutations = []
        changed = self.completed_result()
        changed["builds"]["spectral-norm"]["modes"]["release"]["candidate"][
            "source"
        ]["sha256"] = "0" * 64
        mutations.append(("source", changed))

        changed = self.completed_result()
        changed["builds"]["spectral-norm"]["modes"]["emit-c"]["main"][
            "project"
        ]["copied_source"]["sha256"] = "0" * 64
        mutations.append(("copied-source", changed))

        changed = self.completed_result()
        changed["builds"]["spectral-norm"]["modes"]["release"]["candidate"][
            "project"
        ]["compiler_commit"] = "0" * 40
        mutations.append(("commit", changed))

        changed = self.completed_result()
        changed["builds"]["spectral-norm"]["modes"]["emit-c"]["candidate"][
            "emit_command"
        ]["cwd"] = self.fixture_path("other-checkout")
        mutations.append(("emit-c-cwd", changed))

        changed = self.completed_result()
        changed["builds"]["spectral-norm"]["modes"]["release"]["main"][
            "backend_provenance"
        ]["compile_commands"][0]["cwd"] = self.fixture_path("other-checkout")
        mutations.append(("backend-cwd", changed))

        for label, result in mutations:
            with self.subTest(mutation=label), self.assertRaises(
                benchmark.HarnessError
            ):
                benchmark.validate_build_provenance(result, self.manifest)

    def test_formal_artifacts_cannot_reuse_reference_lane_or_each_other(
        self,
    ) -> None:
        cases = []

        result = self.completed_result()
        build = result["builds"]["spectral-norm"]
        emit = build["modes"]["emit-c"]["candidate"]
        emit["binary"] = copy.deepcopy(build["references"]["binaries"]["c"])
        emit["clang_command"]["argv"][-2] = emit["binary"]["path"]
        emit["clang_command"]["command"] = v1.command_text(
            emit["clang_command"]["argv"]
        )
        forged_binary = emit["binary"]

        def rebind_sample(sample: dict) -> None:
            sample["command_argv"][0] = forged_binary["path"]
            sample["command"] = v1.command_text(sample["command_argv"])
            sample["executable_sha256"] = forged_binary["sha256"]

        for correctness in result["protocols"]["emit-c"]["correctness"]:
            if correctness["id"] == "spectral-norm":
                rebind_sample(
                    correctness["implementations"]["candidate"]["sample"]
                )
        for batch in result["protocols"]["emit-c"]["batches"]:
            workload = next(
                item
                for item in batch["workloads"]
                if item["id"] == "spectral-norm"
            )
            for phase in ("warmups", "samples"):
                for sample in workload[phase]["candidate"]:
                    rebind_sample(sample)
        with mock.patch.object(
            benchmark, "validate_result_prepared_authority"
        ), self.assertRaises(benchmark.HarnessError):
            benchmark.validate_result(result, self.manifest)
        cases.append(("reference-as-candidate", result))

        result = self.completed_result()
        build = result["builds"]["spectral-norm"]
        build["modes"]["release"]["main"]["project"] = copy.deepcopy(
            build["modes"]["release"]["candidate"]["project"]
        )
        cases.append(("candidate-as-main-project", result))

        result = self.completed_result()
        build = result["builds"]["spectral-norm"]
        release = build["modes"]["release"]["candidate"]
        emit = build["modes"]["emit-c"]["candidate"]
        emit["generated_c"] = copy.deepcopy(release["generated_c"])
        emit["generated_c"]["unmodified_after_emit"] = True
        emit["generated_c"].pop("unmodified_after_build")
        emit["binary"] = copy.deepcopy(release["binary"])
        cases.append(("release-as-emit-c", result))

        result = self.completed_result()
        build = result["builds"]["spectral-norm"]
        main = build["modes"]["release"]["main"]
        candidate = build["modes"]["release"]["candidate"]
        main["generated_c"] = copy.deepcopy(candidate["generated_c"])
        main["binary"] = copy.deepcopy(candidate["binary"])
        cases.append(("candidate-as-main-output", result))

        for label, result in cases:
            with self.subTest(reuse=label), self.assertRaises(
                benchmark.HarnessError
            ):
                benchmark.validate_build_provenance(result, self.manifest)

    def test_prepared_bundle_rejects_hardlinked_decisive_artifacts(
        self,
    ) -> None:
        result, bundle = self.prepared_bundle_fixture()
        release = result["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]["binary"]
        emit = result["builds"]["spectral-norm"]["modes"]["emit-c"][
            "candidate"
        ]["binary"]
        release_path = Path(release["path"])
        emit_path = Path(emit["path"])
        emit_path.unlink()
        try:
            os.link(release_path, emit_path)
        except OSError as error:
            self.skipTest(f"hard links unavailable: {error}")
        emit["sha256"] = v1.sha256_file(emit_path)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "multiple hard links"
        ):
            benchmark.write_prepared_bundle(result, bundle, self.manifest)

    def test_forbidden_fast_math_and_replaced_reference_are_rejected(self) -> None:
        result = self.completed_result()
        command = result["builds"]["spectral-norm"]["references"]["commands"][
            "cpp_build"
        ]
        command["argv"].insert(2, "-ffast-math")
        command["command"] = v1.command_text(command["argv"])
        with self.assertRaises(benchmark.HarnessError):
            benchmark.validate_build_provenance(result, self.manifest)
        result = self.completed_result()
        reference = result["builds"]["spectral-norm"]["references"]
        reference["source_files"]["cpp"] = {
            "path": self.fixture_path("replacement.cpp"),
            "sha256": "0" * 64,
        }
        reference["compiled_sources"]["cpp"] = {
            "path": self.fixture_path("replacement-copy.cpp"),
            "sha256": "0" * 64,
        }
        command = reference["commands"]["cpp_build"]
        command["argv"][
            len(benchmark.CLANG_DRIVER_CONFIG_FLAGS)
            + len(benchmark.BASE_CPP_FLAGS)
            + 1
        ] = self.fixture_path("replacement-copy.cpp")
        command["command"] = v1.command_text(command["argv"])
        with self.assertRaisesRegex(benchmark.HarnessError, "manifest-bound"):
            benchmark.validate_build_provenance(result, self.manifest)

    def test_release_backend_rejects_gcc_target_and_missing_fixed_flag(self) -> None:
        for mutation in ("gcc", "target", "flag"):
            result = self.completed_result()
            backend = result["builds"]["spectral-norm"]["modes"]["release"][
                "candidate"
            ]["backend_provenance"]
            if mutation == "gcc":
                gcc = self.fixture_path("tools", "gcc")
                backend["compiler"]["path"] = gcc
                backend["compile_commands"][0]["argv"][0] = gcc
                backend["link_command"]["argv"][0] = gcc
            elif mutation == "target":
                backend["compiler"]["target_triple"] = "arm64-apple-darwin"
            else:
                backend["compile_commands"][0]["argv"].remove("-O3")
            for record in [
                *backend["compile_commands"],
                backend["link_command"],
            ]:
                record["command"] = v1.command_text(record["argv"])
            with self.assertRaises(benchmark.HarnessError):
                benchmark.validate_build_provenance(result, self.manifest)

    def test_release_build_metadata_is_schema_strict_and_recomputed(self) -> None:
        valid = self.completed_result()
        benchmark.validate_result_schema(valid, self.result_schema_path)
        benchmark.validate_build_provenance(
            valid, self.manifest, live_filesystem=False
        )
        release = valid["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]
        self.assertEqual(
            self.build_metadata_bytes(release["build_metadata"]),
            benchmark.build_metadata_canonical_bytes(
                release["build_metadata"]
            ),
        )

        for mutation in ("missing", "extra", "nested-extra"):
            changed = copy.deepcopy(valid)
            formal = changed["builds"]["spectral-norm"]["modes"][
                "release"
            ]["candidate"]
            if mutation == "missing":
                del formal["build_metadata"]
            elif mutation == "extra":
                formal["build_metadata"]["unapproved"] = True
            else:
                formal["build_metadata"]["cache_identity"][
                    "unapproved"
                ] = True
            with self.subTest(schema=mutation), self.assertRaisesRegex(
                benchmark.HarnessError, "Draft 2020-12 schema"
            ):
                benchmark.validate_result_schema(
                    changed, self.result_schema_path
                )

        def mutate_profile(metadata: dict) -> None:
            metadata["selected_profile"] = "debug"

        def mutate_target(metadata: dict) -> None:
            metadata["target_triple"] = "x86_64-apple-darwin-none"

        def mutate_producer(metadata: dict) -> None:
            metadata["producer_executable"]["sha256"] = "0" * 64

        def mutate_query_json(metadata: dict) -> None:
            metadata["cache_identity"]["query_key_json"] += " "

        def mutate_cache_key(metadata: dict) -> None:
            metadata["cache_identity"]["cache_key"] = "0" * 64

        def mutate_cache_input(metadata: dict) -> None:
            metadata["cache_identity"]["inputs"]["query_identity"] = "stale"

        def mutate_compiler(metadata: dict) -> None:
            metadata["compiler"]["path"] = self.fixture_path(
                "tools", "other-clang"
            )

        def mutate_argv(metadata: dict) -> None:
            metadata["compile_commands"][0]["argv"].append("-ffast-math")

        def mutate_generated(metadata: dict) -> None:
            metadata["generated_c"]["sha256"] = "0" * 64

        def mutate_binary(metadata: dict) -> None:
            metadata["binary"]["path"] = self.fixture_path("stale-binary")

        def mutate_sidecar(metadata: dict) -> None:
            metadata["release_provenance"]["sha256"] = "0" * 64

        def mutate_subdocument(metadata: dict) -> None:
            metadata["content_binding"]["canonical_subdocuments"][
                "commands"
            ] = "{}"

        def mutate_binding(metadata: dict) -> None:
            metadata["content_binding"]["sha256"] = "0" * 64

        mutations = {
            "profile": mutate_profile,
            "target": mutate_target,
            "producer": mutate_producer,
            "query-json": mutate_query_json,
            "cache-key": mutate_cache_key,
            "cache-input": mutate_cache_input,
            "compiler": mutate_compiler,
            "argv": mutate_argv,
            "generated-c": mutate_generated,
            "binary": mutate_binary,
            "sidecar": mutate_sidecar,
            "canonical-subdocument": mutate_subdocument,
            "content-binding": mutate_binding,
        }
        for label, mutation in mutations.items():
            changed = copy.deepcopy(valid)
            changed_release = changed["builds"]["spectral-norm"]["modes"][
                "release"
            ]["candidate"]
            metadata = changed_release["build_metadata"]
            mutation(metadata)
            changed_release["build_metadata_sha256"] = v1.sha256_bytes(
                self.build_metadata_bytes(metadata)
            )
            with self.subTest(binding=label), self.assertRaises(
                benchmark.HarnessError
            ):
                benchmark.validate_build_provenance(
                    changed,
                    self.manifest,
                    live_filesystem=False,
                )

    def test_release_metadata_target_binds_nomo_and_backend_domains(self) -> None:
        valid = (
            ("aarch64-apple-darwin-none", "arm64-apple-darwin25.5.0", "Darwin"),
            (
                "aarch64-apple-darwin-none",
                "aarch64-apple-darwin25.5.0",
                "Darwin",
            ),
            ("x86_64-apple-darwin-none", "x86_64-apple-darwin24.6.0", "Darwin"),
            ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "Linux"),
            ("aarch64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "Linux"),
            ("x86_64-pc-windows-msvc", "x86_64-pc-windows-msvc", "Windows"),
            ("aarch64-pc-windows-msvc", "aarch64-pc-windows-msvc", "Windows"),
        )
        for nomo_target, backend_target, host_os in valid:
            with self.subTest(
                nomo=nomo_target, backend=backend_target, host=host_os
            ):
                self.assertTrue(
                    benchmark.nomo_target_matches_backend_target(
                        nomo_target, backend_target, host_os
                    )
                )
        for nomo_target, backend_target, host_os in (
            ("aarch64-apple-darwin-none", "x86_64-apple-darwin25.5.0", "Darwin"),
            ("aarch64-apple-darwin-none", "arm64-apple-darwin25.5.0", "Linux"),
            ("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "Linux"),
            ("x86_64-pc-windows-msvc", "x86_64-pc-windows-msvc", "Darwin"),
            ("arm64-apple-darwin-none", "arm64-apple-darwin25.5.0", "Darwin"),
            ("aarch64-apple-darwin-none", "arm64-apple-darwin.", "Darwin"),
            ("aarch64-apple-darwin-none", "arm64-apple-darwin25..5", "Darwin"),
            (
                "x86_64-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu6.8",
                "Linux",
            ),
            (
                "x86_64-pc-windows-msvc",
                "x86_64-pc-windows-msvc19",
                "Windows",
            ),
            (
                "aarch64-unknown-linux-gnu",
                "arm64-unknown-linux-gnu",
                "Linux",
            ),
            (
                "aarch64-pc-windows-msvc",
                "arm64-pc-windows-msvc",
                "Windows",
            ),
        ):
            with self.subTest(
                nomo=nomo_target, backend=backend_target, host=host_os
            ):
                self.assertFalse(
                    benchmark.nomo_target_matches_backend_target(
                        nomo_target, backend_target, host_os
                    )
                )

    def test_release_build_metadata_rejects_stale_and_lane_exchange(self) -> None:
        valid = self.completed_result()
        workload = valid["builds"]["spectral-norm"]["modes"]["release"]

        exchanged = copy.deepcopy(valid)
        exchanged_workload = exchanged["builds"]["spectral-norm"]["modes"][
            "release"
        ]
        exchanged_workload["candidate"]["build_metadata"] = copy.deepcopy(
            exchanged_workload["main"]["build_metadata"]
        )
        exchanged_workload["candidate"]["build_metadata_sha256"] = (
            exchanged_workload["main"]["build_metadata_sha256"]
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "producer identity|binding"
        ):
            benchmark.validate_build_provenance(
                exchanged, self.manifest, live_filesystem=False
            )

        stale = copy.deepcopy(valid)
        candidate = stale["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]
        candidate["binary"]["sha256"] = "f" * 64
        candidate["backend_provenance"]["binary"] = copy.deepcopy(
            candidate["binary"]
        )
        with self.assertRaisesRegex(
            benchmark.HarnessError, "output or release-sidecar binding"
        ):
            benchmark.validate_build_provenance(
                stale, self.manifest, live_filesystem=False
            )

        self.assertNotEqual(
            workload["candidate"]["build_metadata"]["content_binding"][
                "sha256"
            ],
            workload["main"]["build_metadata"]["content_binding"]["sha256"],
        )

    def test_prepared_release_metadata_raw_bytes_are_authoritative(self) -> None:
        result, bundle = self.prepared_bundle_fixture()
        formal = result["builds"]["spectral-norm"]["modes"]["release"][
            "candidate"
        ]
        metadata_path = Path(formal["build_metadata_path"])
        canonical = metadata_path.read_bytes()
        self.assertEqual(
            canonical,
            benchmark.build_metadata_canonical_bytes(
                formal["build_metadata"]
            ),
        )

        mutations = {
            "noncanonical": (
                json.dumps(
                    formal["build_metadata"],
                    indent=4,
                    sort_keys=True,
                    ensure_ascii=False,
                )
                + "\n"
            ).encode("utf-8"),
            "duplicate": canonical.replace(
                b"{\n", b'{\n  "schema": 1,\n', 1
            ),
        }
        for label, raw in mutations.items():
            with self.subTest(raw=label):
                metadata_path.write_bytes(raw)
                changed = copy.deepcopy(result)
                changed_formal = changed["builds"]["spectral-norm"]["modes"][
                    "release"
                ]["candidate"]
                changed_formal["build_metadata_sha256"] = v1.sha256_bytes(
                    raw
                )
                inventory = benchmark.prepared_file_inventory(bundle)
                with self.assertRaisesRegex(
                    benchmark.HarnessError,
                    "canonical JSON|duplicate JSON object key",
                ):
                    benchmark.validate_prepared_bundle_files(
                        changed, bundle, inventory
                    )
                metadata_path.write_bytes(canonical)

        metadata_path.write_bytes(canonical.replace(b'"release"', b'"debug"', 1))
        with self.assertRaisesRegex(
            benchmark.HarnessError, "added, removed, or changed"
        ):
            benchmark.load_prepared_bundle(bundle)

    def test_windows_exact_argv_does_not_expect_libm(self) -> None:
        result = self.project_result_to_producer_os(
            self.completed_result(), "Windows"
        )
        for workload_id in ("spectral-norm", "n-body"):
            build = result["builds"][workload_id]
            for name in ("c_build", "cpp_build", "semantic-c_build"):
                record = build["references"]["commands"][name]
                self.assertNotIn("-lm", record["argv"])
            for mode in benchmark.FORMAL_BUILD_MODES:
                for lane in ("candidate", "main"):
                    formal = build["modes"][mode][lane]
                    if mode == "emit-c":
                        records = [formal["clang_command"]]
                    else:
                        records = [formal["backend_provenance"]["link_command"]]
                    for record in records:
                        self.assertNotIn("-lm", record["argv"])
        benchmark.validate_build_provenance(
            result, self.manifest, live_filesystem=False
        )

    def test_same_recorded_build_evidence_replays_on_every_reviewer(
        self,
    ) -> None:
        evidence = self.completed_result()
        self.assertEqual(evidence["provenance"]["host"]["os"], "Linux")
        for reviewer_os in ("Linux", "Darwin", "Windows"):
            with self.subTest(reviewer_os=reviewer_os), mock.patch.object(
                benchmark,
                "sanitized_build_environment",
                side_effect=AssertionError(
                    "offline replay consulted the reviewer environment"
                ),
            ), mock.patch.object(
                benchmark.platform, "system", return_value=reviewer_os
            ):
                benchmark.validate_build_provenance(
                    copy.deepcopy(evidence),
                    self.manifest,
                    live_filesystem=False,
                )

    def test_offline_validate_cli_replays_canonical_result_without_live_tools(
        self,
    ) -> None:
        result = self.correctness_only_result()
        downloaded_suite = benchmark.PurePosixPath(
            "/downloaded/linux/nomo/performance/benchmarksgame"
        )
        result["provenance"]["manifest_path"] = str(
            downloaded_suite / "manifest-v2.json"
        )
        manifest_workloads = {
            workload["id"]: workload for workload in self.manifest["workloads"]
        }
        for workload_id, build in result["builds"].items():
            workload = manifest_workloads[workload_id]
            references = build["references"]
            for lane in benchmark.REFERENCE_LANES:
                references["source_files"][lane]["path"] = str(
                    downloaded_suite / workload["sources"][lane]["path"]
                )
            for command in references["commands"].values():
                command["cwd"] = "/downloaded/linux/nomo"
            for index, source in enumerate(
                (
                    workload["sources"]["nomo"]["path"],
                    workload["sources"]["nomo"]["project_manifest"],
                )
            ):
                references["source_files"]["nomo"][index]["path"] = str(
                    downloaded_suite / source
                )
        for correctness in result["correctness"]:
            workload = manifest_workloads[correctness["id"]]
            correctness["fixture_path"] = str(
                downloaded_suite
                / workload["fixtures"]["correctness"]["path"]
            )
        artifact = Path(self.temporary.name) / "downloaded-linux-result.json"
        v1.write_result(artifact, result)
        with mock.patch.object(
            benchmark,
            "inspect_toolchains",
            side_effect=AssertionError("offline validation probed toolchains"),
        ), mock.patch.object(
            benchmark,
            "sanitized_build_environment",
            side_effect=AssertionError("offline validation read reviewer env"),
        ), mock.patch.object(
            benchmark,
            "run_build_capture",
            side_effect=AssertionError("offline validation executed a tool"),
        ), mock.patch.object(
            benchmark,
            "_environment",
            side_effect=AssertionError(
                "offline validation rebuilt reviewer runtime env"
            ),
        ):
            path, replayed = benchmark.validate_artifact_offline(
                str(artifact), str(self.manifest_path)
            )
        self.assertEqual(path, artifact)
        self.assertEqual(replayed, result)

        prepared, bundle = self.prepared_bundle_fixture()
        for reviewer_os in ("Linux", "Darwin", "Windows"):
            with self.subTest(
                prepared_reviewer_os=reviewer_os
            ), mock.patch.object(
                benchmark.platform, "system", return_value=reviewer_os
            ), mock.patch.object(
                benchmark,
                "inspect_toolchains",
                side_effect=AssertionError(
                    "offline validation probed toolchains"
                ),
            ), mock.patch.object(
                benchmark,
                "sanitized_build_environment",
                side_effect=AssertionError(
                    "offline validation read reviewer env"
                ),
            ), mock.patch.object(
                benchmark,
                "run_build_capture",
                side_effect=AssertionError(
                    "offline validation executed a tool"
                ),
            ), mock.patch.object(
                benchmark,
                "_environment",
                side_effect=AssertionError(
                    "offline validation rebuilt reviewer runtime env"
                ),
            ):
                prepared_path, replayed_prepared = (
                    benchmark.validate_artifact_offline(
                        str(bundle), str(self.manifest_path)
                    )
                )
            self.assertEqual(prepared_path, bundle)
            self.assertEqual(replayed_prepared, prepared)

    def test_completed_artifact_replays_cross_host_without_live_authority(
        self,
    ) -> None:
        result = self.completed_result()
        downloaded_suite = benchmark.PurePosixPath(
            "/downloaded/linux/nomo/performance/benchmarksgame"
        )
        result["provenance"]["manifest_path"] = str(
            downloaded_suite / "manifest-v2.json"
        )
        workloads = {
            workload["id"]: workload for workload in self.manifest["workloads"]
        }
        for workload_id, build in result["builds"].items():
            workload = workloads[workload_id]
            references = build["references"]
            for lane in benchmark.REFERENCE_LANES:
                references["source_files"][lane]["path"] = str(
                    downloaded_suite / workload["sources"][lane]["path"]
                )
            for command in references["commands"].values():
                command["cwd"] = "/downloaded/linux/nomo"
            for build_mode in benchmark.FORMAL_BUILD_MODES:
                for lane in ("candidate", "main"):
                    formal = build["modes"][build_mode][lane]
                    project = formal["project"]
                    source_path = str(
                        downloaded_suite
                        / workload["sources"]["nomo"]["path"]
                    )
                    project["source"]["path"] = source_path
                    formal["source"]["path"] = source_path
                    project["project_manifest"]["path"] = str(
                        downloaded_suite
                        / workload["sources"]["nomo"]["project_manifest"]
                    )
        for protocol in result["protocols"].values():
            for correctness in protocol["correctness"]:
                workload = workloads[correctness["id"]]
                correctness["fixture_path"] = str(
                    downloaded_suite
                    / workload["fixtures"]["correctness"]["path"]
                )
            for batch in protocol["batches"]:
                for measured in batch["workloads"]:
                    workload = workloads[measured["id"]]
                    measured["fixture_path"] = str(
                        downloaded_suite
                        / workload["fixtures"]["performance"]["path"]
                    )
        qualification_path = Path(
            result["provenance"]["environment_qualification"][
                "qualification_path"
            ]
        )
        qualification_path.unlink()
        result["provenance"]["environment_qualification"][
            "qualification_path"
        ] = "/downloaded/linux/authority/environment.json"
        artifact = (
            Path(self.temporary.name) / "downloaded-completed-linux-result.json"
        )
        v1.write_result(artifact, result)
        for reviewer_os in ("Linux", "Darwin", "Windows"):
            with self.subTest(reviewer_os=reviewer_os), mock.patch.object(
                benchmark.platform, "system", return_value=reviewer_os
            ), mock.patch.object(
                benchmark,
                "inspect_toolchains",
                side_effect=AssertionError(
                    "offline validation probed toolchains"
                ),
            ), mock.patch.object(
                benchmark,
                "sanitized_build_environment",
                side_effect=AssertionError(
                    "offline validation read reviewer env"
                ),
            ), mock.patch.object(
                benchmark,
                "run_build_capture",
                side_effect=AssertionError(
                    "offline validation executed a tool"
                ),
            ), mock.patch.object(
                benchmark,
                "_environment",
                side_effect=AssertionError(
                    "offline validation rebuilt reviewer runtime env"
                ),
            ):
                path, replayed = benchmark.validate_artifact_offline(
                    str(artifact), str(self.manifest_path)
                )
            self.assertEqual(path, artifact)
            self.assertEqual(replayed, result)

        tampered = copy.deepcopy(result)
        tampered["provenance"]["environment_qualification"]["checks"][
            "power_mode"
        ]["value"] = "forged"
        tampered_artifact = Path(self.temporary.name) / "tampered-completed.json"
        v1.write_result(tampered_artifact, tampered)
        with self.assertRaisesRegex(
            benchmark.HarnessError, "canonical qualification file SHA"
        ):
            benchmark.validate_artifact_offline(
                str(tampered_artifact), str(self.manifest_path)
            )

    def test_offline_replay_cross_os_status_matrix_uses_no_live_authority(
        self,
    ) -> None:
        success = self.correctness_only_result()
        failure = self.correctness_build_failure_result()
        formal_unavailable = self.formal_unavailable_result()
        prepared = self.prepared_only_result()
        states = {
            "success": success,
            "failure": failure,
            "formal-unavailable": formal_unavailable,
            "prepared": prepared,
        }
        completed_by_os = {}
        for producer_os in ("Linux", "Darwin", "Windows"):
            completed = self.project_result_to_producer_os(
                self.completed_result(), producer_os
            )
            self.rebind_static_authorization(completed, eligible=True)
            authorization = completed["provenance"][
                "environment_qualification"
            ]
            bindings = authorization["expected_bindings"]
            snapshot_counter = iter(range(1, 100))
            linux_factory = self.dynamic_snapshot_factory()
            for protocol in completed["protocols"].values():
                for batch in protocol["batches"]:
                    batch["static_authorization_sha256"] = authorization[
                        "qualification_sha256"
                    ]
                    if producer_os == "Darwin":
                        before_index = next(snapshot_counter)
                        after_index = next(snapshot_counter)
                        batch["dynamic_environment_before"] = (
                            self.darwin_dynamic_snapshot(
                                bindings["authority_host_sha256"],
                                before_index,
                                "2026-07-28T00:00:05+00:00",
                            )
                        )
                        batch["dynamic_environment_after"] = (
                            self.darwin_dynamic_snapshot(
                                bindings["authority_host_sha256"],
                                after_index,
                                "2026-07-28T00:00:50+00:00",
                            )
                        )
                    elif producer_os == "Windows":
                        before_index = next(snapshot_counter)
                        after_index = next(snapshot_counter)
                        batch["dynamic_environment_before"] = (
                            self.windows_dynamic_snapshot(
                                bindings["authority_host_sha256"],
                                before_index,
                                "2026-07-28T00:00:05+00:00",
                            )
                        )
                        batch["dynamic_environment_after"] = (
                            self.windows_dynamic_snapshot(
                                bindings["authority_host_sha256"],
                                after_index,
                                "2026-07-28T00:00:50+00:00",
                            )
                        )
                    elif producer_os == "Linux":
                        batch["dynamic_environment_before"] = linux_factory(
                            bindings["authority_host_sha256"]
                        )
                        batch["dynamic_environment_after"] = linux_factory(
                            bindings["authority_host_sha256"]
                        )
            completed_by_os[producer_os] = completed

        valid_cases = 0
        for producer_os in ("Linux", "Darwin", "Windows"):
            producer_states = {
                name: self.project_result_to_producer_os(
                    state, producer_os
                )
                for name, state in states.items()
            }
            for state in producer_states.values():
                self.rebind_static_authorization(state, eligible=False)
            producer_states["completed"] = completed_by_os[producer_os]
            for state_name, result in producer_states.items():
                benchmark.validate_result_schema(
                    result, self.result_schema_path
                )
                for reviewer_os in ("Linux", "Darwin", "Windows"):
                    with self.subTest(
                        producer_os=producer_os,
                        state=state_name,
                        reviewer_os=reviewer_os,
                    ), mock.patch.object(
                        benchmark.platform,
                        "system",
                        return_value=reviewer_os,
                    ), mock.patch.object(
                        benchmark,
                        "inspect_toolchains",
                        side_effect=AssertionError(
                            "offline replay probed toolchains"
                        ),
                    ), mock.patch.object(
                        benchmark,
                        "sanitized_build_environment",
                        side_effect=AssertionError(
                            "offline replay read the reviewer build environment"
                        ),
                    ), mock.patch.object(
                        benchmark,
                        "_environment",
                        side_effect=AssertionError(
                            "offline replay rebuilt the reviewer runtime environment"
                        ),
                    ), mock.patch.object(
                        benchmark,
                        "run_build_capture",
                        side_effect=AssertionError(
                            "offline replay executed a build tool"
                        ),
                    ), mock.patch.object(
                        benchmark,
                        "windows_system_directory",
                        side_effect=AssertionError(
                            "offline replay queried the reviewer system directory"
                        ),
                    ), mock.patch.object(
                        benchmark,
                        "validate_live_release_lane_authority",
                        side_effect=AssertionError(
                            "offline replay inspected a live release lane"
                        ),
                    ), mock.patch.object(
                        benchmark.Path,
                        "is_file",
                        side_effect=AssertionError(
                            "offline replay inspected a reviewer file"
                        ),
                    ), mock.patch.object(
                        benchmark.Path,
                        "resolve",
                        side_effect=AssertionError(
                            "offline replay resolved a reviewer path"
                        ),
                    ), mock.patch.object(
                        v1,
                        "sha256_file",
                        side_effect=AssertionError(
                            "offline replay hashed a reviewer file"
                        ),
                    ), mock.patch.object(
                        benchmark.subprocess,
                        "run",
                        side_effect=AssertionError(
                            "offline replay launched a reviewer process"
                        ),
                    ), mock.patch.object(
                        v1,
                        "git_capture",
                        side_effect=AssertionError(
                            "offline replay inspected a reviewer repository"
                        ),
                    ):
                        benchmark.validate_result(
                            result,
                            self.manifest,
                            offline_replay=True,
                        )
                        valid_cases += 1
        self.assertEqual(valid_cases, 45)

    def test_workflow_runs_correctness_and_upload_after_python_test_failure(
        self,
    ) -> None:
        for workflow in (
            REPOSITORY_ROOT / ".github" / "workflows" / "pr-smoke.yml",
            REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml",
        ):
            text = workflow.read_text(encoding="utf-8")
            with self.subTest(workflow=workflow.name):
                correctness = text.index(
                    "- name: Run v2 small-input correctness gate"
                )
                upload = text.index(
                    "- name: Upload v2 correctness evidence",
                    correctness,
                )
                section = text[correctness:upload]
                self.assertIn("if: ${{ always() }}", section)
                upload_section = text[upload : upload + 500]
                self.assertIn("if: ${{ always() }}", upload_section)
                self.assertIn(".json", upload_section)
                self.assertIn(".log", upload_section)

    def synthetic_workload(
        self, workload_id: str, candidate_wall: int, comparator_wall: int
    ) -> dict:
        samples = {
            lane: [
                {
                    "wall_ns": (
                        candidate_wall if lane == "candidate" else comparator_wall
                    )
                }
            ]
            * 30
            for lane in benchmark.TIMED_LANES
        }
        return {"id": workload_id, "samples": samples}

    def stability_workloads(
        self,
        *,
        candidate: list[float],
        c: list[float],
        cpp: list[float],
        main: list[float],
        go: list[float],
    ) -> list[dict]:
        values = {
            "candidate": candidate,
            "c": c,
            "cpp": cpp,
            "main": main,
            "go": go,
        }
        return [
            {
                "id": workload,
                "samples": {
                    lane: [{"wall_ns": wall} for wall in walls]
                    for lane, walls in values.items()
                },
            }
            for workload in benchmark.WORKLOAD_IDS
        ]

    def stability_summary(self, valid: bool, issues: list[str]) -> dict:
        return {
            "reference_drift": [],
            "diagnostic_reference_drift": [],
            "paired_ratio_rsd": [],
            "issues": issues,
            "warnings": [],
            "valid": valid,
            "outliers_removed": False,
        }

    def stub_measurement(
        self,
        _manifest: dict,
        _suite_root: Path,
        workload: dict,
        _binaries: dict,
        _collector: object,
        build_mode: str,
        batch_index: int,
        attempt_index: int,
        evidence: dict,
    ) -> dict:
        evidence.update({
            "id": workload["id"],
            "build_mode": build_mode,
            "batch_index": batch_index,
            "attempt_index": attempt_index,
        })
        return evidence

    def measured_sample(
        self,
        *,
        wall_ns: int,
        phase: str,
        build_mode: str,
        workload_id: str,
        lane: str,
        input_value: str,
        fixture_sha256: str,
        batch_index: int,
        attempt_index: int,
        order_position: int,
        warmup_index: int | None = None,
        block_index: int | None = None,
    ) -> dict:
        binary = self.binary_record(workload_id, build_mode, lane)
        argv = [binary["path"], input_value]
        manifest_workload = next(
            item
            for item in self.manifest["workloads"]
            if item["id"] == workload_id
        )
        fixture_kind = (
            "correctness"
            if input_value == manifest_workload["correctness_input"]
            else "performance"
        )
        fixture_bytes = (
            self.suite_root
            / manifest_workload["fixtures"][fixture_kind]["path"]
        ).read_bytes()
        self.assertEqual(v1.sha256_bytes(fixture_bytes), fixture_sha256)
        sample = {
            "started_at_utc": "2026-07-28T00:00:20+00:00",
            "finished_at_utc": "2026-07-28T00:00:21+00:00",
            "command_argv": argv,
            "command": v1.command_text(argv),
            "environment": benchmark._environment(
                {"GOMAXPROCS": "1"} if lane == "go" else {}
            )[1],
            "collector": benchmark.collector_descriptor_for_host("Linux")[
                "id"
            ],
            "stdout": "captured-and-verified",
            "wall_ns": wall_ns,
            "user_cpu_ns": wall_ns,
            "system_cpu_ns": 0,
            "cpu_total_ns": wall_ns,
            "peak_rss_bytes": 1,
            "exit_code": 0,
            "stdout_raw_sha256": fixture_sha256,
            "stdout_normalized_sha256": fixture_sha256,
            "stdout_raw_base64": base64.b64encode(fixture_bytes).decode(
                "ascii"
            ),
            "stdout_normalization": benchmark.STDOUT_NORMALIZATION,
            "executable_sha256": binary["sha256"],
            "phase": phase,
            "build_mode": build_mode,
            "batch_index": batch_index,
            "attempt_index": attempt_index,
            "order_position": order_position,
        }
        if warmup_index is not None:
            sample["warmup_index"] = warmup_index
        if block_index is not None:
            sample["block_index"] = block_index
        return sample

    def measured_workload(
        self, workload_id: str, build_mode: str, batch_index: int, attempt_index: int
    ) -> dict:
        schedule = benchmark.williams_schedule()
        manifest_workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        input_value = manifest_workload["performance_input"]
        fixture_sha = manifest_workload["fixtures"]["performance"]["sha256"]
        warmups = {lane: [] for lane in benchmark.TIMED_LANES}
        samples = {lane: [] for lane in benchmark.TIMED_LANES}
        for warmup_index, order in enumerate(schedule[:2], start=1):
            for lane in benchmark.TIMED_LANES:
                warmups[lane].append(
                    self.measured_sample(
                        wall_ns=100,
                        phase="warmup",
                        build_mode=build_mode,
                        workload_id=workload_id,
                        lane=lane,
                        input_value=input_value,
                        fixture_sha256=fixture_sha,
                        batch_index=batch_index,
                        attempt_index=attempt_index,
                        warmup_index=warmup_index,
                        order_position=order.index(lane) + 1,
                    )
                )
        for block_index, order in enumerate(schedule, start=1):
            for lane in benchmark.TIMED_LANES:
                samples[lane].append(
                    self.measured_sample(
                        wall_ns=100,
                        phase="sample",
                        build_mode=build_mode,
                        workload_id=workload_id,
                        lane=lane,
                        input_value=input_value,
                        fixture_sha256=fixture_sha,
                        batch_index=batch_index,
                        attempt_index=attempt_index,
                        block_index=block_index,
                        order_position=order.index(lane) + 1,
                    )
                )
        return {
            "id": workload_id,
            "build_mode": build_mode,
            "performance_input": input_value,
            "fixture_path": str(
                self.suite_root / manifest_workload["fixtures"]["performance"]["path"]
            ),
            "fixture_sha256": fixture_sha,
            "warmup_orders": schedule[:2],
            "warmups": warmups,
            "block_schedule": [
                {"block_index": index, "order": order}
                for index, order in enumerate(schedule, start=1)
            ],
            "samples": samples,
            "collection_status": "completed",
            "failure_reason": None,
        }

    def bind_formal_candidate_binary(
        self,
        result: dict,
        workload_id: str,
        build_mode: str,
        executable: Path,
    ) -> dict:
        formal = result["builds"][workload_id]["modes"][build_mode][
            "candidate"
        ]
        target = Path(formal["binary"]["path"])
        target.parent.mkdir(parents=True, exist_ok=True)
        if executable == Path("/bin/sleep"):
            target.write_text(
                "#!/bin/sh\nexec /bin/sleep 5\n", encoding="utf-8"
            )
            target.chmod(0o755)
        elif executable == Path("/usr/bin/printf"):
            target.write_text(
                "#!/bin/sh\nexec /usr/bin/printf x\n", encoding="utf-8"
            )
            target.chmod(0o755)
        else:
            shutil.copyfile(executable, target)
            target.chmod(executable.stat().st_mode)
        binary = {
            "path": str(target.resolve()),
            "sha256": v1.sha256_file(target),
        }
        formal["binary"] = binary
        workload = next(
            item
            for item in self.manifest["workloads"]
            if item["id"] == workload_id
        )
        link_flags = ["-lm"] if workload.get("link_math") else []
        if build_mode == "release":
            backend = formal["backend_provenance"]
            backend["binary"] = binary
            previous_environment = copy.deepcopy(
                backend["link_command"]["environment"]
            )
            backend["link_command"] = self.full_command(
                [
                    backend["compiler"]["path"],
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    backend["objects"][0]["path"],
                    "-o",
                    binary["path"],
                    *link_flags,
                ],
                cwd=formal["command"]["cwd"],
            )
            backend["link_command"]["environment"] = previous_environment
        else:
            previous_environment = copy.deepcopy(
                formal["clang_command"]["environment"]
            )
            formal["clang_command"] = self.full_command(
                [
                    self.fixture_path("tools", "clang"),
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_C_FLAGS,
                    formal["generated_c"]["path"],
                    "-o",
                    binary["path"],
                    *link_flags,
                ],
                cwd=formal["emit_command"]["cwd"],
            )
            formal["clang_command"]["environment"] = previous_environment
        self.rebind_release_metadata(result)
        return binary

    def real_failed_partial_workload(
        self,
        result: dict,
        workload_id: str,
        build_mode: str,
        attempt_index: int,
        executable: Path,
        collector: benchmark.ProcessCollector,
        failure_kind: str,
    ) -> dict:
        manifest_workload = next(
            item
            for item in self.manifest["workloads"]
            if item["id"] == workload_id
        )
        fixture = (
            self.suite_root
            / manifest_workload["fixtures"]["performance"]["path"]
        )
        timeout = 0.25 if failure_kind == "timeout" else 30.0
        with self.assertRaises(benchmark.SampleCollectionError) as failure:
            collector.run(
                [str(executable.resolve()), manifest_workload["performance_input"]],
                expected_stdout=fixture.read_bytes(),
                timeout_seconds=timeout,
            )
        sample = copy.deepcopy(failure.exception.record)
        sample.update(
            {
                "phase": "warmup",
                "build_mode": build_mode,
                "batch_index": 1,
                "attempt_index": attempt_index,
                "warmup_index": 1,
                "order_position": 1,
                "executable_sha256": v1.sha256_file(executable),
            }
        )
        self.assertEqual(sample["failure_kind"], failure_kind)
        warmups = {lane: [] for lane in benchmark.TIMED_LANES}
        warmups["candidate"].append(sample)
        return {
            "id": workload_id,
            "build_mode": build_mode,
            "performance_input": manifest_workload["performance_input"],
            "fixture_path": str(fixture),
            "fixture_sha256": manifest_workload["fixtures"]["performance"][
                "sha256"
            ],
            "warmup_orders": benchmark.williams_schedule()[:2],
            "warmups": warmups,
            "block_schedule": [],
            "samples": {lane: [] for lane in benchmark.TIMED_LANES},
            "collection_status": "failed",
            "failure_reason": sample["failure_message"],
        }

    def bind_collector(self, workload: dict, collector_id: str) -> None:
        for phase in ("warmups", "samples"):
            for lane_samples in workload[phase].values():
                for sample in lane_samples:
                    sample["collector"] = collector_id

    def correctness_item(self, workload_id: str, build_mode: str) -> dict:
        lanes = (
            benchmark.CORRECTNESS_BASELINE_LANES
            if build_mode == "baseline-emit-c"
            else benchmark.CORRECTNESS_FORMAL_LANES
        )
        workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        fixture_sha = workload["fixtures"]["correctness"]["sha256"]
        implementations = {}
        for lane in lanes:
            sample = self.measured_sample(
                wall_ns=100,
                phase="sample",
                build_mode=(
                    "release" if build_mode == "baseline-emit-c" else build_mode
                ),
                workload_id=workload_id,
                lane=lane,
                input_value=workload["correctness_input"],
                fixture_sha256=fixture_sha,
                batch_index=1,
                attempt_index=1,
                block_index=1,
                order_position=1,
            )
            sample.pop("phase")
            sample.pop("build_mode")
            sample.pop("batch_index")
            sample.pop("attempt_index")
            sample.pop("block_index")
            sample.pop("order_position")
            implementations[lane] = {
                "passed": True,
                "stdout_normalized_sha256": fixture_sha,
                "sample": sample,
            }
        return {
            "id": workload_id,
            "build_mode": build_mode,
            "input": workload["correctness_input"],
            "fixture_path": str(
                self.suite_root / workload["fixtures"]["correctness"]["path"]
            ),
            "fixture_sha256": fixture_sha,
            "lanes": list(lanes),
            "attempted_lanes": list(lanes),
            "status": "completed",
            "failure_reason": None,
            "implementations": implementations,
        }

    def completed_protocol(self, build_mode: str) -> dict:
        batches = []
        snapshot_factory = self.dynamic_snapshot_factory()
        authorization = self.static_authorization_stub()
        for batch_index in (1, 2):
            workloads = [
                self.measured_workload(
                    workload, build_mode, batch_index, batch_index
                )
                for workload in benchmark.WORKLOAD_IDS
            ]
            stability = benchmark.batch_stability(
                workloads, self.manifest["methodology"]["batch_invalidation"]
            )
            evaluation = benchmark.evaluate_batch(
                workloads, self.manifest["thresholds"], True
            )
            batches.append(
                {
                    "build_mode": build_mode,
                    "batch_index": batch_index,
                    "attempt_index": batch_index,
                    "started_at_utc": "2026-07-28T00:00:00+00:00",
                    "finished_at_utc": "2026-07-28T00:01:00+00:00",
                    "static_authorization_sha256": authorization[
                        "qualification_sha256"
                    ],
                    "dynamic_environment_before": snapshot_factory(
                        self.qualification_bindings()["authority_host_sha256"]
                    ),
                    "dynamic_environment_after": snapshot_factory(
                        self.qualification_bindings()["authority_host_sha256"]
                    ),
                    "workloads": workloads,
                    "status": "completed",
                    "invalidation_reasons": [],
                    "stability": stability,
                    "evaluation": evaluation,
                }
            )
        return {
            "build_mode": build_mode,
            "status": "completed",
            "reason": "fixture",
            "correctness": [
                self.correctness_item(workload, build_mode)
                for workload in benchmark.WORKLOAD_IDS
            ],
            "batches": batches,
            "verdict": "pass",
        }

    def qualification_bindings(self) -> dict:
        return {
            "authority_host_sha256": "1" * 64,
            "reference_toolchains_sha256": "2" * 64,
            "frozen_source_lock_sha256": "3" * 64,
            "candidate_commit": "4" * 40,
            "candidate_nomo_sha256": "5" * 64,
            "main_commit": "6" * 40,
            "main_nomo_sha256": "7" * 64,
            "prepared_bundle_sha256": "e" * 64,
        }

    def qualified_check(self, value: object) -> dict:
        return {
            "status": "qualified",
            "value": value,
            "source": "fixture",
            "evidence": {
                "kind": "file",
                "captured_at_utc": "2026-07-28T00:00:00+00:00",
                "value_sha256": benchmark.canonical_json_sha256(value),
            },
        }

    def static_authorization_stub(self) -> dict:
        return {
            "kind": "canonical-host-static-authorization-v1",
            "status": "eligible",
            "eligible": True,
            "policy": "fail-closed",
            "qualification_path": str(Path(self.temporary.name) / "environment.json"),
            "qualification_sha256": "8" * 64,
            "canonical_host_id": "fixture-host",
            "captured_at_utc": "2026-07-28T00:00:00+00:00",
            "checks": {},
            "missing_or_unqualified": [],
            "provided_bindings": self.qualification_bindings(),
            "expected_bindings": self.qualification_bindings(),
            "binding_mismatches": [],
            "dynamic_policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            "reason": "fixture",
        }

    def dynamic_snapshot_factory(self):
        counter = iter(range(1, 100))

        def capture(authority_host_sha256: str, policy=None) -> dict:
            index = next(counter)
            parsed_values = {
                "power_mode": (
                    "sysfs",
                    {"AC": "1"},
                    {"ac_power": True},
                    {},
                ),
                "low_power_mode": (
                    "sysfs",
                    ["performance"],
                    {"enabled": False},
                    {},
                ),
                "frequency_governor": (
                    "sysfs",
                    ["performance"],
                    {"governors": ["performance"]},
                    {},
                ),
                "thermal_state": (
                    "sysfs",
                    [40.0],
                    {
                        "temperatures_celsius": [40.0],
                        "maximum_celsius": 40.0,
                    },
                    {},
                ),
                "concurrent_load": (
                    "os.getloadavg",
                    {
                        "load_average": [0.02, 0.01, 0.01],
                        "logical_cores": 2,
                    },
                    {
                        "load_average": [0.02, 0.01, 0.01],
                        "logical_cores": 2,
                        "one_minute_per_logical_core": 0.01,
                        "failure_threshold": 1.0,
                    },
                    {},
                ),
                "swap": {"used_bytes": 0},
                "affinity": (
                    "os.sched_getaffinity",
                    [0, 1],
                    {"cpus": [0, 1]},
                    {},
                ),
            }
            parsed_values["swap"] = (
                "procfs",
                "SwapTotal: 0 kB\nSwapFree: 0 kB\n",
                {"used_bytes": 0},
                {},
            )
            observations = {
                name: {
                    "status": "qualified",
                    "source": source,
                    "raw": {
                        "sha256": v1.sha256_bytes(
                            (
                                raw_value
                                if isinstance(raw_value, str)
                                else json.dumps(
                                    raw_value,
                                    sort_keys=True,
                                    separators=(",", ":"),
                                )
                            ).encode("utf-8")
                        ),
                        "length_bytes": len(
                            (
                                raw_value
                                if isinstance(raw_value, str)
                                else json.dumps(
                                    raw_value,
                                    sort_keys=True,
                                    separators=(",", ":"),
                                )
                            ).encode("utf-8")
                        ),
                        "text": (
                            raw_value
                            if isinstance(raw_value, str)
                            else json.dumps(
                                raw_value,
                                sort_keys=True,
                                separators=(",", ":"),
                            )
                        ),
                    },
                    "parsed": parsed,
                    "reason": "fixture",
                    **extra,
                }
                for name, (source, raw_value, parsed, extra) in parsed_values.items()
            }
            body = {
                "schema": 1,
                "captured_at_utc": (
                    "2026-07-28T00:00:05+00:00"
                    if index % 2 == 1
                    else "2026-07-28T00:00:50+00:00"
                ),
                "monotonic_ns": index,
                "authority_host_sha256": authority_host_sha256,
                "observed_host_sha256": authority_host_sha256,
                "observations": observations,
                "policy": policy or benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                "eligible": True,
                "reason": "fixture",
            }
            return {
                **body,
                "snapshot_sha256": benchmark.canonical_json_sha256(body),
            }

        return capture

    def fixture_path(self, *parts: str) -> str:
        return str(Path(self.temporary.name).resolve().joinpath(*parts))

    def project_windows_paths_as_linux_artifact(self, value):
        if isinstance(value, dict):
            projected = {
                key: self.project_windows_paths_as_linux_artifact(item)
                for key, item in value.items()
            }
            if (
                (
                    isinstance(projected.get("argv"), list)
                    or isinstance(projected.get("command_argv"), list)
                )
                and "command" in projected
            ):
                command_argv = projected.get(
                    "argv", projected.get("command_argv")
                )
                projected["command"] = v1.command_text(command_argv)
            return projected
        if isinstance(value, list):
            return [
                self.project_windows_paths_as_linux_artifact(item)
                for item in value
            ]
        if not isinstance(value, str) or ";" in value or "\n" in value:
            return value
        path = benchmark.PureWindowsPath(value)
        if not path.is_absolute():
            return value
        drive = path.drive.rstrip(":\\/").casefold() or "unc"
        relative_parts = path.parts[1:]
        return str(
            benchmark.PurePosixPath(
                "/recorded-windows-producer", drive, *relative_parts
            )
        )

    def project_result_to_producer_os(
        self, value: dict, producer_os: str
    ) -> dict:
        projected = copy.deepcopy(value)
        if producer_os == "Windows":
            textual_keys = {
                "command",
                "error",
                "reason",
                "stdout",
                "stderr",
                "text",
                "version_output",
            }

            def windows_path(text: str) -> str:
                path = benchmark.PurePosixPath(text)
                if (
                    len(path.parts) >= 3
                    and path.parts[1] == "recorded-windows-producer"
                    and len(path.parts[2]) == 1
                ):
                    return str(
                        benchmark.PureWindowsPath(
                            f"{path.parts[2].upper()}:/",
                            *path.parts[3:],
                        )
                    )
                return str(
                    benchmark.PureWindowsPath(
                        "C:/recorded-benchmark-producer",
                        *path.parts[1:],
                    )
                )

            def convert(item, key: str | None = None):
                if isinstance(item, dict):
                    converted = {
                        child_key: convert(child, child_key)
                        for child_key, child in item.items()
                    }
                    argv = converted.get(
                        "argv", converted.get("command_argv")
                    )
                    if (
                        isinstance(argv, list)
                        and "command" in converted
                    ):
                        converted["command"] = v1.command_text(argv)
                    return converted
                if isinstance(item, list):
                    return [convert(child, key) for child in item]
                if not isinstance(item, str) or key in textual_keys:
                    return item
                if key == "PATH":
                    return ";".join(
                        windows_path(path)
                        for path in item.split(":")
                        if path
                    )
                if benchmark.PurePosixPath(item).is_absolute():
                    return windows_path(item)
                return item

            projected = convert(projected)
        projected["provenance"]["host"]["os"] = producer_os
        projected["provenance"]["host"]["architecture"] = (
            "arm64" if producer_os == "Darwin" else "x86_64"
        )
        backend_target = {
            "Darwin": "arm64-apple-darwin25.0.0",
            "Linux": "x86_64-unknown-linux-gnu",
            "Windows": "x86_64-pc-windows-msvc",
        }[producer_os]
        for compiler_name in ("clang", "clangxx"):
            projected["provenance"]["toolchains"][compiler_name][
                "target_triple"
            ] = backend_target
        for build in projected.get("builds", {}).values():
            for formal in build.get("modes", {}).get(
                "release", {}
            ).values():
                backend = formal.get("backend_provenance")
                if isinstance(backend, dict):
                    backend["compiler"]["target_triple"] = backend_target
        projected["provenance"]["collector"] = (
            benchmark.collector_descriptor_for_host(producer_os)
        )
        collector_id = projected["provenance"]["collector"]["id"]

        def bind_collector(item) -> None:
            if isinstance(item, dict):
                if isinstance(item.get("collector"), str):
                    item["collector"] = collector_id
                for child in item.values():
                    bind_collector(child)
            elif isinstance(item, list):
                for child in item:
                    bind_collector(child)

        bind_collector(projected)
        if producer_os == "Windows":
            retained = {
                "GOENV": "off",
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": (
                    r"C:\recorded-benchmark-producer\tools"
                    r";C:\Windows\System32"
                ),
                "SystemRoot": r"C:\Windows",
                "TEMP": r"C:\Windows\Temp",
                "TMP": r"C:\Windows\Temp",
                "WINDIR": r"C:\Windows",
            }
            build_projection = {
                "retained": retained,
                "cleared": [
                    name
                    for name in benchmark.COMPILER_AFFECTING_ENVIRONMENT
                    if name not in retained
                ],
                "cleared_values_recorded": False,
            }
            runtime = {
                "default": {
                    "LANG": "C",
                    "LC_ALL": "C",
                    "SystemRoot": r"C:\Windows",
                    "TEMP": r"C:\Windows\Temp",
                    "TMP": r"C:\Windows\Temp",
                    "WINDIR": r"C:\Windows",
                }
            }
            runtime["go"] = {
                **runtime["default"],
                "GOMAXPROCS": "1",
            }

            def remove_libm(item) -> None:
                if isinstance(item, dict):
                    argv = item.get("argv")
                    if isinstance(argv, list):
                        item["argv"] = [
                            argument
                            for argument in argv
                            if argument != "-lm"
                        ]
                        if "command" in item:
                            item["command"] = v1.command_text(item["argv"])
                    for child in item.values():
                        remove_libm(child)
                elif isinstance(item, list):
                    for child in item:
                        remove_libm(child)

            remove_libm(projected)
            for failure in projected.get("build_failures", []):
                previous = failure["output_path"]
                if not previous.lower().endswith(".exe"):
                    failure["output_path"] = previous + ".exe"
                    command = failure["command"]
                    command["argv"] = [
                        (
                            failure["output_path"]
                            if argument == previous
                            else argument
                        )
                        for argument in command["argv"]
                    ]
                    command["command"] = v1.command_text(command["argv"])
        else:
            projected = self.project_windows_paths_as_linux_artifact(
                projected
            )
            build_projection = {
                "retained": {
                    "GOENV": "off",
                    "HOME": "/home/benchmark-authority",
                    "LANG": "C",
                    "LC_ALL": "C",
                    "PATH": "/usr/bin:/bin",
                    "TMPDIR": "/tmp",
                },
                "cleared": [
                    name
                    for name in benchmark.COMPILER_AFFECTING_ENVIRONMENT
                    if name
                    not in {
                        "GOENV",
                        "HOME",
                        "LANG",
                        "LC_ALL",
                        "PATH",
                        "TMPDIR",
                    }
                ],
                "cleared_values_recorded": False,
            }
            runtime = self.synthetic_linux_runtime_environments()
        projected["provenance"]["toolchains"][
            "build_environment"
        ] = build_projection
        self.bind_build_command_environments(
            projected, build_projection
        )
        self.rebind_release_metadata(projected)
        self.bind_runtime_environments(projected, runtime)
        return projected

    def rebind_release_metadata(self, result: dict) -> None:
        for build in result.get("builds", {}).values():
            for formal in build.get("modes", {}).get(
                "release", {}
            ).values():
                if formal.get("kind") != "real-nomo-release":
                    continue
                previous = formal.get("build_metadata", {})
                producer_size = (
                    previous.get("producer_executable", {}).get(
                        "size_bytes", 1
                    )
                )
                metadata = self.release_build_metadata(
                    formal,
                    producer_size_bytes=producer_size,
                )
                formal["build_metadata"] = metadata
                formal["build_metadata_sha256"] = v1.sha256_bytes(
                    self.build_metadata_bytes(metadata)
                )

    def synthetic_linux_runtime_environments(self) -> dict:
        default = {
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "TMPDIR": "/tmp",
        }
        return {
            "default": default,
            "go": {**default, "GOMAXPROCS": "1"},
        }

    def bind_synthetic_build_environment(self, result: dict) -> None:
        retained = {
            "GOENV": "off",
            "HOME": "/home/benchmark-authority",
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "TMPDIR": "/tmp",
        }
        projection = {
            "retained": retained,
            "cleared": [
                name
                for name in benchmark.COMPILER_AFFECTING_ENVIRONMENT
                if name not in retained
            ],
            "cleared_values_recorded": False,
        }
        self.bind_build_command_environments(result, projection)
        result["provenance"]["toolchains"]["build_environment"] = projection

    def bind_build_command_environments(
        self, result: dict, projection: dict
    ) -> None:
        override_names = {
            "CARGO_TARGET_DIR",
            "CARGO_HOME",
            "RUSTC",
            "GOCACHE",
            "GOMODCACHE",
        }

        def bind(value) -> None:
            if isinstance(value, dict):
                environment = value.get("environment")
                if (
                    isinstance(environment, dict)
                    and isinstance(environment.get("retained"), dict)
                    and isinstance(value.get("argv"), list)
                ):
                    overrides = {
                        key: item
                        for key, item in environment["retained"].items()
                        if key in override_names
                    }
                    rebound = copy.deepcopy(projection)
                    rebound["retained"].update(overrides)
                    rebound["retained"] = dict(
                        sorted(rebound["retained"].items())
                    )
                    rebound["cleared"] = [
                        name
                        for name in benchmark.COMPILER_AFFECTING_ENVIRONMENT
                        if name not in rebound["retained"]
                    ]
                    value["environment"] = rebound
                for item in value.values():
                    bind(item)
            elif isinstance(value, list):
                for item in value:
                    bind(item)

        bind(result)

    def bind_synthetic_runtime_environments(self, result: dict) -> None:
        environments = self.synthetic_linux_runtime_environments()
        self.bind_runtime_environments(result, environments)

    def bind_runtime_environments(
        self, result: dict, environments: dict
    ) -> None:
        result["provenance"]["toolchains"]["runtime_environments"] = (
            copy.deepcopy(environments)
        )

        def bind_correctness(items: list[dict]) -> None:
            for item in items:
                for lane, implementation in item.get(
                    "implementations", {}
                ).items():
                    implementation["sample"]["environment"] = copy.deepcopy(
                        environments["go" if lane == "go" else "default"]
                    )

        bind_correctness(result.get("correctness", []))
        for protocol in result.get("protocols", {}).values():
            bind_correctness(protocol.get("correctness", []))
            for batch in protocol.get("batches", []):
                for workload in batch.get("workloads", []):
                    for phase in ("warmups", "samples"):
                        for lane, samples in workload.get(
                            phase, {}
                        ).items():
                            for sample in samples:
                                sample["environment"] = copy.deepcopy(
                                    environments[
                                        "go" if lane == "go" else "default"
                                    ]
                                )

    def rebind_static_authorization(
        self,
        result: dict,
        *,
        eligible: bool,
    ) -> None:
        provenance = result["provenance"]
        bindings = benchmark.qualification_bindings(
            provenance["host"],
            provenance["toolchains"],
            provenance["source_lock"],
            result["release_lanes"],
            provenance.get("prepared_bundle_sha256"),
        )
        if not eligible:
            provenance["environment_qualification"] = (
                benchmark.environment_qualification(
                    self.manifest, None, bindings
                )
            )
            return
        checks = {
            check: self.qualified_check(f"qualified:{check}")
            for check in benchmark.EXPECTED_REQUIRED_CHECKS
        }
        checks["canonical_host_identity"] = self.qualified_check(
            bindings["authority_host_sha256"]
        )
        checks["toolchain_identity"] = self.qualified_check(
            bindings["reference_toolchains_sha256"]
        )
        checks["frozen_source_lock"] = self.qualified_check(
            bindings["frozen_source_lock_sha256"]
        )
        document = {
            "schema": 1,
            "canonical_host_id": "offline-producer-host",
            "captured_at_utc": "2026-07-28T00:00:00+00:00",
            "dynamic_policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            "bindings": bindings,
            "checks": checks,
        }
        producer_os = provenance["host"]["os"]
        qualification_path = (
            r"C:\recorded-benchmark-producer\authority\environment.json"
            if producer_os == "Windows"
            else "/recorded-benchmark-producer/authority/environment.json"
        )
        canonical = (
            json.dumps(document, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        provenance["environment_qualification"] = (
            benchmark.derive_environment_qualification(
                self.manifest,
                document,
                qualification_path,
                v1.sha256_bytes(canonical),
                bindings,
            )
        )

    def darwin_dynamic_snapshot(
        self,
        authority_host_sha256: str,
        monotonic_ns: int,
        captured_at_utc: str,
    ) -> dict:
        environment = {
            "LC_ALL": "C",
            "LANG": "C",
            "PATH": "/usr/bin:/usr/sbin:/bin",
        }

        def command_observation(
            observation_id: str,
            executable: str,
            arguments: list[str],
            text: str,
        ) -> dict:
            observation = {
                "status": "qualified",
                "source": "command",
                "command_argv": [executable, *arguments],
                "command_identity": {
                    "path": executable,
                    "realpath": executable,
                    "sha256": "9" * 64,
                    "version_output": None,
                },
                "environment": copy.deepcopy(environment),
                "exit_code": 0,
                "raw": benchmark._raw_text_evidence(text),
                "parsed": None,
                "reason": "offline Darwin producer fixture",
            }
            observation["parsed"] = (
                benchmark.parse_dynamic_observation_from_raw(
                    observation_id,
                    observation,
                    benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                )
            )
            self.assertTrue(
                benchmark.dynamic_observation_is_qualified(
                    observation_id,
                    observation,
                    benchmark.DYNAMIC_ENVIRONMENT_POLICY,
                    "Darwin",
                    "arm64",
                )
            )
            return observation

        load = {
            "load_average": [0.02, 0.01, 0.01],
            "logical_cores": 2,
        }
        observations = {
            "power_mode": command_observation(
                "power_mode",
                benchmark.DARWIN_PMSET,
                ["-g", "batt"],
                "Now drawing from 'AC Power'\n",
            ),
            "low_power_mode": command_observation(
                "low_power_mode",
                benchmark.DARWIN_PMSET,
                ["-g"],
                " lowpowermode 0\n",
            ),
            "frequency_governor": command_observation(
                "frequency_governor",
                benchmark.DARWIN_PMSET,
                ["-g", "therm"],
                "\n".join(benchmark.DARWIN_PMSET_NO_RECORDED_LINES)
                + "\n",
            ),
            "thermal_state": command_observation(
                "thermal_state",
                benchmark.DARWIN_OSASCRIPT,
                [
                    "-l",
                    "JavaScript",
                    "-e",
                    benchmark.DARWIN_THERMAL_STATE_SCRIPT,
                ],
                "0\n",
            ),
            "concurrent_load": {
                "status": "qualified",
                "source": "os.getloadavg",
                "raw": benchmark._raw_json_evidence(load),
                "parsed": {
                    **load,
                    "one_minute_per_logical_core": 0.01,
                    "failure_threshold": 1.0,
                },
                "reason": "offline Darwin producer fixture",
            },
            "swap": command_observation(
                "swap",
                benchmark.DARWIN_SYSCTL,
                ["-n", "vm.swapusage"],
                "total = 0.00M used = 0.00M free = 0.00M\n",
            ),
            "affinity": {
                "status": "qualified",
                "source": "system-api",
                "raw": benchmark._raw_json_evidence(
                    {"supported": False}
                ),
                "parsed": {
                    "supported": False,
                    "enforced": False,
                },
                "reason": "offline Darwin producer fixture",
            },
        }
        body = {
            "schema": 1,
            "captured_at_utc": captured_at_utc,
            "monotonic_ns": monotonic_ns,
            "authority_host_sha256": authority_host_sha256,
            "observed_host_sha256": authority_host_sha256,
            "observations": observations,
            "policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            "eligible": True,
            "reason": "offline Darwin producer fixture",
        }
        return {
            **body,
            "snapshot_sha256": benchmark.canonical_json_sha256(body),
        }

    def windows_dynamic_snapshot(
        self,
        authority_host_sha256: str,
        monotonic_ns: int,
        captured_at_utc: str,
    ) -> dict:
        policy = benchmark.DYNAMIC_ENVIRONMENT_POLICY
        architecture = "x86_64"
        power_state = {
            "schema": 1,
            "api": "GetSystemPowerStatus",
            "success": True,
            "ac_line_status": 1,
            "battery_flag": 128,
            "battery_life_percent": 255,
            "system_status_flag": 0,
            "battery_life_time_seconds": 0xFFFFFFFF,
            "battery_full_life_time_seconds": 0xFFFFFFFF,
        }
        processor_state = {
            "schema": 1,
            "api": "CallNtPowerInformation",
            "information_level": {
                "name": "ProcessorInformation",
                "value": 11,
            },
            "ntstatus": 0,
            "processor_group_count": 1,
            "logical_processor_count": 2,
            "processors": [
                {
                    "number": number,
                    "max_mhz": 3200,
                    "current_mhz": 1600,
                    "mhz_limit": 3200,
                    "max_idle_state": 3,
                    "current_idle_state": 1,
                }
                for number in range(2)
            ],
        }
        system_state = {
            "schema": 1,
            "api": "CallNtPowerInformation",
            "information_level": {
                "name": "SystemPowerInformation",
                "value": 12,
            },
            "ntstatus": 0,
            "max_idleness_allowed_percent": 90,
            "idleness_percent": 99,
            "time_remaining_seconds": 300,
            "cooling_mode": 0,
        }
        pagefile_state = {
            "schema": 1,
            "api": "EnumPageFilesW",
            "success": True,
            "page_size_bytes": 4096,
            "pagefiles": [
                {
                    "path": r"C:\pagefile.sys",
                    "total_size_pages": 1024,
                    "total_in_use_pages": 0,
                    "peak_usage_pages": 0,
                }
            ],
        }
        affinity_state = {
            "schema": 1,
            "api": "GetProcessAffinityMask",
            "success": True,
            "pointer_width_bits": 64,
            "processor_group_count": 1,
            "process_mask_hex": "0x3",
            "system_mask_hex": "0x3",
        }
        observations = {
            "power_mode": benchmark.windows_dynamic_observation(
                "power_mode",
                "GetSystemPowerStatus",
                power_state,
                policy,
                architecture,
            ),
            "low_power_mode": benchmark.windows_dynamic_observation(
                "low_power_mode",
                "GetSystemPowerStatus",
                power_state,
                policy,
                architecture,
            ),
            "frequency_governor": (
                benchmark.windows_dynamic_observation(
                    "frequency_governor",
                    "CallNtPowerInformation:ProcessorInformation",
                    processor_state,
                    policy,
                    architecture,
                )
            ),
            "thermal_state": benchmark.windows_dynamic_observation(
                "thermal_state",
                "CallNtPowerInformation:ProcessorInformation",
                processor_state,
                policy,
                architecture,
            ),
            "concurrent_load": benchmark.windows_dynamic_observation(
                "concurrent_load",
                "CallNtPowerInformation:SystemPowerInformation",
                system_state,
                policy,
                architecture,
            ),
            "swap": benchmark.windows_dynamic_observation(
                "swap",
                "EnumPageFilesW",
                pagefile_state,
                policy,
                architecture,
            ),
            "affinity": benchmark.windows_dynamic_observation(
                "affinity",
                "GetProcessAffinityMask",
                affinity_state,
                policy,
                architecture,
            ),
        }
        self.assertTrue(
            all(
                observation["status"] == "qualified"
                for observation in observations.values()
            )
        )
        body = {
            "schema": 1,
            "captured_at_utc": captured_at_utc,
            "monotonic_ns": monotonic_ns,
            "authority_host_sha256": authority_host_sha256,
            "observed_host_sha256": authority_host_sha256,
            "observations": observations,
            "policy": policy,
            "eligible": True,
            "reason": "offline Windows producer fixture",
        }
        return {
            **body,
            "snapshot_sha256": benchmark.canonical_json_sha256(body),
        }

    def correctness_build_failure_result(self) -> dict:
        result = self.correctness_only_result()
        bundle = Path(self.temporary.name) / "offline-failure-bundle"
        source = (
            bundle
            / "build"
            / "spectral-norm"
            / "references"
            / "go.go"
        )
        source.parent.mkdir(parents=True, exist_ok=True)
        frozen = (
            self.suite_root
            / self.manifest["workloads"][0]["sources"]["go"]["path"]
        )
        shutil.copy2(frozen, source)
        output = bundle / "bin" / "spectral-norm-go"
        caches = {
            key: str((source.parent / directory).resolve())
            for key, directory in benchmark.GO_BUILD_CACHE_DIRECTORIES.items()
        }
        command = benchmark.failed_build_command_record(
            [
                result["provenance"]["toolchains"]["go"]["path"],
                "build",
                "-o",
                str(output.resolve()),
                str(source.resolve()),
            ],
            REPOSITORY_ROOT,
            benchmark.sanitized_build_environment(caches)[1],
            "2026-07-28T00:00:00+00:00",
            1,
            b"",
            b"fixture compiler failure",
            exit_code=1,
            timed_out=False,
            error="command exited with status 1",
        )
        failure = benchmark.workload_build_failure_record(
            "spectral-norm",
            "go",
            "reference-build",
            source,
            output,
            command,
        )
        result["status"] = "ineligible"
        result["correctness"] = []
        result["builds"] = {}
        result["build_failures"] = [failure]
        result = self.project_windows_paths_as_linux_artifact(result)
        self.bind_synthetic_build_environment(result)
        self.rebind_static_authorization(result, eligible=False)
        return result

    def prepared_only_result(self) -> dict:
        result = self.completed_result()
        result["mode"] = "prepare"
        result["status"] = "prepared"
        result["claims"]["claim_eligible"] = False
        result["correctness"] = []
        result["protocols"] = {
            build_mode: {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": "prepared but not measured",
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }
            for build_mode in benchmark.FORMAL_BUILD_MODES
        }
        result["overall_verdict"] = "not_evaluated"
        root = Path(self.temporary.name) / "offline-prepared-bundle"
        result["provenance"]["prepared_bundle_path"] = str(
            root / "prepared-bundle.json"
        )
        result["provenance"]["qualification_request_path"] = str(
            root / "qualification-request.json"
        )
        self.rebind_static_authorization(result, eligible=False)
        return result

    def formal_unavailable_result(self) -> dict:
        result = self.completed_result()
        result["mode"] = "prepare"
        result["status"] = "unavailable"
        result["claims"]["claim_eligible"] = False
        result["correctness"] = []
        result["builds"] = {}
        result["build_failures"] = []
        result["release_lanes"]["candidate"]["capabilities"]["release"][
            "status"
        ] = "unavailable"
        result["release_lanes"]["candidate"]["capabilities"]["release"][
            "reason"
        ] = "formal release capability unavailable"
        result["protocols"] = {
            build_mode: {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": "formal build mode unavailable",
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }
            for build_mode in benchmark.FORMAL_BUILD_MODES
        }
        result["overall_verdict"] = "not_evaluated"
        result["provenance"]["prepared_bundle_sha256"] = None
        result["provenance"]["prepared_bundle_path"] = None
        result["provenance"]["qualification_request_path"] = None
        self.rebind_static_authorization(result, eligible=False)
        return result

    def bind_live_execution_authority(self, toolchains: dict) -> None:
        toolchains["build_environment"] = (
            benchmark.sanitized_build_environment()[1]
        )
        toolchains["runtime_environments"] = {
            "default": benchmark._environment({})[1],
            "go": benchmark._environment({"GOMAXPROCS": "1"})[1],
        }

    def command_record(
        self,
        argv: list[str],
        approved_environment_overrides: dict[str, str] | None = None,
    ) -> dict:
        copied = list(argv)
        return {
            "argv": copied,
            "command": v1.command_text(copied),
            "environment": benchmark.sanitized_build_environment(
                approved_environment_overrides
            )[1],
        }

    def full_command(
        self,
        argv: list[str],
        cwd: str | None = None,
        approved_environment_overrides: dict[str, str] | None = None,
    ) -> dict:
        return {
            **self.command_record(argv, approved_environment_overrides),
            "cwd": cwd,
            "duration_ns": 1,
            "exit_code": 0,
        }

    def binary_record(self, workload_id: str, build_mode: str, lane: str) -> dict:
        if lane in benchmark.REFERENCE_LANES or lane == "nomo-baseline":
            return {
                "path": self.fixture_path(
                    f"{workload_id}-reference-{lane}"
                ),
                "sha256": ("1" if lane != "go" else "2") * 64,
            }
        return {
            "path": self.fixture_path(
                workload_id,
                build_mode,
                lane,
                "project",
                "build",
                "bin",
                "program",
            ),
            "sha256": ("3" if lane == "candidate" else "4") * 64,
        }

    def reference_build(self, workload_id: str = "spectral-norm") -> dict:
        workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        link_flags = ["-lm"] if workload.get("link_math") else []
        compiled = {
            lane: {
                "path": self.fixture_path(
                    f"{workload_id}-{lane}"
                    f"{'.cpp' if lane == 'cpp' else '.go' if lane == 'go' else '.c'}"
                ),
                "sha256": workload["sources"][lane]["sha256"],
            }
            for lane in benchmark.REFERENCE_LANES
        }
        binaries = {
            lane: self.binary_record(workload_id, "reference", lane)
            for lane in benchmark.REFERENCE_LANES
        }
        c = [
            self.fixture_path("tools", "clang"),
            *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
            *benchmark.BASE_C_FLAGS,
            compiled["c"]["path"],
            "-o",
            binaries["c"]["path"],
            *link_flags,
        ]
        cpp = [
            self.fixture_path("tools", "clang++"),
            *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
            *benchmark.BASE_CPP_FLAGS,
            compiled["cpp"]["path"],
            "-o",
            binaries["cpp"]["path"],
            *link_flags,
        ]
        semantic_c = [
            self.fixture_path("tools", "clang"),
            *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
            *benchmark.BASE_C_FLAGS,
            compiled["semantic-c"]["path"],
            "-o",
            binaries["semantic-c"]["path"],
            *link_flags,
        ]
        return {
            "source_files": {
                lane: {
                    "path": str(
                        self.suite_root / workload["sources"][lane]["path"]
                    ),
                    "sha256": workload["sources"][lane]["sha256"],
                }
                for lane in benchmark.REFERENCE_LANES
            },
            "compiled_sources": compiled,
            "binaries": binaries,
            "commands": {
                "c_build": self.command_record(c),
                "cpp_build": self.command_record(cpp),
                "semantic-c_build": self.command_record(semantic_c),
                "go_build": self.command_record(
                    [
                        self.fixture_path("tools", "go"),
                        "build",
                        "-o",
                        binaries["go"]["path"],
                        compiled["go"]["path"],
                    ],
                    {
                        key: str(
                            (
                                Path(compiled["go"]["path"]).parent
                                / directory
                            ).resolve()
                        )
                        for key, directory in benchmark.GO_BUILD_CACHE_DIRECTORIES.items()
                    },
                ),
            }
        }

    def formal_build(
        self, lane: str, build_mode: str, workload_id: str = "spectral-norm"
    ) -> dict:
        workload = next(
            item
            for item in self.manifest["workloads"]
            if item["id"] == workload_id
        )
        commit = ("a" if lane == "candidate" else "b") * 40
        nomo_sha = ("c" if lane == "candidate" else "d") * 64
        nomo_path = self.fixture_path(lane, "target", "release", "nomo")
        project = self.fixture_path(workload_id, build_mode, lane, "project")
        generated_c = f"{project}/build/c/main.c"
        binary = self.binary_record(workload_id, build_mode, lane)
        nomo_source = workload["sources"]["nomo"]
        source_path = str(
            (self.suite_root / nomo_source["path"]).resolve()
        )
        manifest_path = str(
            (
                self.suite_root / nomo_source["project_manifest"]
            ).resolve()
        )
        base = {
            "repository": {"commit": commit},
            "nomo": {"path": nomo_path, "sha256": nomo_sha},
            "source": {
                "path": source_path,
                "sha256": nomo_source["sha256"],
            },
            "project": {
                "path": project,
                "source_relative_path": nomo_source["path"],
                "source": {
                    "path": source_path,
                    "sha256": nomo_source["sha256"],
                },
                "project_manifest_relative_path": nomo_source[
                    "project_manifest"
                ],
                "project_manifest": {
                    "path": manifest_path,
                    "sha256": nomo_source[
                        "project_manifest_sha256"
                    ],
                },
                "copied_source": {
                    "path": f"{project}/src/main.nomo",
                    "sha256": nomo_source["sha256"],
                },
                "copied_project_manifest": {
                    "path": f"{project}/nomo.toml",
                    "sha256": nomo_source[
                        "project_manifest_sha256"
                    ],
                },
                "compiler_checkout": self.fixture_path(lane),
                "compiler_commit": commit,
            },
            "lane": lane,
            "binary": binary,
            "compile_time_excluded_from_run_time": True,
        }
        if build_mode == "release":
            release = {
                **base,
                "kind": "real-nomo-release",
                "command": self.full_command(
                    [nomo_path, "build", project, "--release"],
                    cwd=self.fixture_path(lane),
                ),
                "stdout": "",
                "stderr": "",
                "generated_c": {
                    "path": generated_c,
                    "sha256": "2" * 64,
                    "unmodified_after_build": True,
                },
                "backend_provenance_path": f"{project}/build/release-provenance.json",
                "backend_provenance_sha256": "5" * 64,
                "backend_provenance": self.release_backend(
                    workload_id, generated_c, binary, lane
                ),
                "emit_c_fallback_used": False,
            }
            metadata = self.release_build_metadata(release)
            release["build_metadata_path"] = (
                f"{project}/build/nomo-build-metadata.json"
            )
            release["build_metadata_sha256"] = v1.sha256_bytes(
                self.build_metadata_bytes(metadata)
            )
            release["build_metadata"] = metadata
            return release
        return {
            **base,
            "kind": "nomo-emit-c-clang",
            "emit_command": self.full_command(
                [nomo_path, "build", project, "--emit-c"],
                cwd=self.fixture_path(lane),
            ),
            "emit_stdout": "",
            "emit_stderr": "",
            "clang_command": self.full_command(
                [
                    self.fixture_path("tools", "clang"),
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_C_FLAGS,
                    generated_c,
                    "-o",
                    binary["path"],
                    *(
                        ["-lm"]
                        if next(
                            item
                            for item in self.manifest["workloads"]
                            if item["id"] == workload_id
                        ).get("link_math")
                        else []
                    ),
                ],
                cwd=self.fixture_path(lane),
            ),
            "clang_stdout": "",
            "clang_stderr": "",
            "generated_c": {
                "path": generated_c,
                "sha256": "3" * 64,
                "unmodified_after_emit": True,
            },
            "release_artifact_reused": False,
        }

    @staticmethod
    def build_metadata_bytes(metadata: dict) -> bytes:
        return (
            json.dumps(
                metadata,
                indent=2,
                sort_keys=True,
                ensure_ascii=False,
            )
            + "\n"
        ).encode("utf-8")

    def release_build_metadata(
        self,
        release: dict,
        *,
        producer_size_bytes: int = 1,
    ) -> dict:
        producer_sha = release["nomo"]["sha256"]
        producer = {
            "schema": 1,
            "path": release["nomo"]["path"],
            "realpath": release["nomo"]["path"],
            "sha256": producer_sha,
            "size_bytes": producer_size_bytes,
            "package_version": "0.0.0-fixture",
        }
        backend_target = release["backend_provenance"]["compiler"][
            "target_triple"
        ]
        if backend_target.startswith(("arm64-", "aarch64-apple-")):
            target = "aarch64-apple-darwin-none"
        elif backend_target.startswith("x86_64-apple-"):
            target = "x86_64-apple-darwin-none"
        elif backend_target.startswith("aarch64-unknown-linux-gnu"):
            target = "aarch64-unknown-linux-gnu"
        elif backend_target.startswith("x86_64-unknown-linux-gnu"):
            target = "x86_64-unknown-linux-gnu"
        elif backend_target.startswith("aarch64-pc-windows-msvc"):
            target = "aarch64-pc-windows-msvc"
        elif backend_target.startswith("x86_64-pc-windows-msvc"):
            target = "x86_64-pc-windows-msvc"
        else:
            raise AssertionError(
                f"unsupported fixture backend target: {backend_target}"
            )
        toolchain_config = (
            "profile-release:"
            f"compiler-exe-sha256:{producer_sha}:"
            f"runtime-exe-sha256:{producer_sha}:"
            "pipeline-1:"
            f"driver-{'1' * 64}:"
            f"cflags-{'2' * 64}:"
            f"sqlite-3.50.4:{'3' * 64}:{'4' * 64}:{'5' * 64}:{'6' * 64}"
        )
        query_key = {
            "schema": 1,
            "toolchain": producer["package_version"],
            "target": target,
            "namespace": "codegen-c",
            "identity": f"project-fixture:{toolchain_config}",
            "fingerprint": f"sha256:{'7' * 64}",
        }
        query_json = json.dumps(
            query_key,
            ensure_ascii=False,
            separators=(",", ":"),
        )
        cache_input_order = (
            "profile",
            "target_triple",
            "producer_executable_sha256",
            "compiler_revision",
            "runtime_revision",
            "pass_pipeline_version",
            "toolchain_config_version",
            "toolchain_config",
            "toolchain_config_sha256",
            "query_schema",
            "query_toolchain",
            "query_target",
            "query_namespace",
            "query_identity",
            "query_fingerprint",
        )
        cache_inputs = {
            "profile": "release",
            "target_triple": target,
            "producer_executable_sha256": producer_sha,
            "compiler_revision": f"exe-sha256:{producer_sha}",
            "runtime_revision": f"exe-sha256:{producer_sha}",
            "pass_pipeline_version": "1",
            "toolchain_config_version": "1",
            "toolchain_config": toolchain_config,
            "toolchain_config_sha256": v1.sha256_bytes(
                toolchain_config.encode("utf-8")
            ),
            "query_schema": "1",
            "query_toolchain": query_key["toolchain"],
            "query_target": query_key["target"],
            "query_namespace": query_key["namespace"],
            "query_identity": query_key["identity"],
            "query_fingerprint": query_key["fingerprint"],
        }
        cache = {
            "schema": 1,
            "algorithm": "sha256",
            "formula": "sha256(UTF-8 bytes of query_key_json)",
            "input_order": list(cache_input_order),
            "inputs": cache_inputs,
            "cache_key": v1.sha256_bytes(query_json.encode("utf-8")),
            "query_key": query_key,
            "query_key_json": query_json,
        }
        compiler = copy.deepcopy(release["backend_provenance"]["compiler"])
        compile_commands = copy.deepcopy(
            release["backend_provenance"]["compile_commands"]
        )
        link_command = copy.deepcopy(
            release["backend_provenance"]["link_command"]
        )
        generated = {
            "path": release["generated_c"]["path"],
            "sha256": release["generated_c"]["sha256"],
        }
        binary = copy.deepcopy(release["binary"])
        sidecar = {
            "path": release["backend_provenance_path"],
            "sha256": release["backend_provenance_sha256"],
        }
        commands_document = {
            "compile_commands": compile_commands,
            "link_command": link_command,
            "combined_compile_link_command": None,
        }

        def compact(value) -> str:
            return json.dumps(
                value,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )

        subdocuments = {
            "producer_identity": compact(producer),
            "compiler_identity": compact(compiler),
            "commands": compact(commands_document),
        }
        content_input_order = (
            "profile",
            "target_triple",
            "cache_key",
            "producer_identity_sha256",
            "compiler_identity_sha256",
            "commands_sha256",
            "generated_c_path",
            "generated_c_sha256",
            "binary_path",
            "binary_sha256",
            "release_provenance_path",
            "release_provenance_sha256",
        )
        content_inputs = {
            "profile": "release",
            "target_triple": target,
            "cache_key": cache["cache_key"],
            "producer_identity_sha256": v1.sha256_bytes(
                subdocuments["producer_identity"].encode("utf-8")
            ),
            "compiler_identity_sha256": v1.sha256_bytes(
                subdocuments["compiler_identity"].encode("utf-8")
            ),
            "commands_sha256": v1.sha256_bytes(
                subdocuments["commands"].encode("utf-8")
            ),
            "generated_c_path": generated["path"],
            "generated_c_sha256": generated["sha256"],
            "binary_path": binary["path"],
            "binary_sha256": binary["sha256"],
            "release_provenance_path": sidecar["path"],
            "release_provenance_sha256": sidecar["sha256"],
        }
        framed = bytearray()
        for part in (
            "nomo-build-metadata-content-binding-v1",
            *(
                component
                for name in content_input_order
                for component in (name, content_inputs[name])
            ),
        ):
            encoded = part.encode("utf-8")
            framed.extend(len(encoded).to_bytes(8, "big"))
            framed.extend(encoded)
        return {
            "schema": 1,
            "selected_profile": "release",
            "target_triple": target,
            "producer_executable": producer,
            "cache_identity": cache,
            "content_binding": {
                "schema": 1,
                "algorithm": "sha256",
                "domain": "nomo-build-metadata-content-binding-v1",
                "formula": (
                    "sha256(concat(u64be(length(utf8(part))), utf8(part)) "
                    "for domain, then each ordered input name and value)"
                ),
                "input_order": list(content_input_order),
                "inputs": content_inputs,
                "canonical_subdocuments": subdocuments,
                "sha256": v1.sha256_bytes(bytes(framed)),
            },
            "compiler": compiler,
            "complete_argv": True,
            "compile_commands": compile_commands,
            "link_command": link_command,
            "combined_compile_link_command": None,
            "generated_c": generated,
            "binary": binary,
            "release_provenance": sidecar,
        }

    def release_backend(
        self, workload_id: str, generated_c: str, binary: dict, lane: str
    ) -> dict:
        workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        object_path = f"{Path(generated_c).parent}/main.o"
        compiler = {
            "path": self.fixture_path("tools", "clang"),
            "realpath": self.fixture_path("tools", "clang"),
            "sha256": "9" * 64,
            "version_output": "Apple clang version 21.0.0",
            "target_triple": "x86_64-unknown-linux-gnu",
        }
        return {
            "schema": 1,
            "complete_argv": True,
            "compiler": compiler,
            "objects": [{"path": object_path, "sha256": "a" * 64}],
            "compile_commands": [
                self.full_command(
                    [
                        compiler["path"],
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        *benchmark.BASE_C_FLAGS,
                        "-c",
                        generated_c,
                        "-o",
                        object_path,
                    ],
                    cwd=self.fixture_path(lane),
                )
            ],
            "link_command": self.full_command(
                [
                    compiler["path"],
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    object_path,
                    "-o",
                    binary["path"],
                    *(["-lm"] if workload.get("link_math") else []),
                ],
                cwd=self.fixture_path(lane),
            ),
            "generated_c": {"path": generated_c, "sha256": "2" * 64},
            "binary": binary,
        }

    def schema_reference_build(
        self,
        workload_id: str = "spectral-norm",
        include_nomo_baseline: bool = False,
    ) -> dict:
        reference = self.reference_build(workload_id)
        generated_c = None
        if include_nomo_baseline:
            workload = next(
                item
                for item in self.manifest["workloads"]
                if item["id"] == workload_id
            )
            nomo_source = workload["sources"]["nomo"]
            project = Path(
                self.fixture_path(
                    "reference-build",
                    workload_id,
                    "nomo-baseline-project",
                )
            )
            generated_path = str(
                (project / "build" / "c" / "main.c").resolve()
            )
            baseline_binary = self.binary_record(
                workload_id, "reference", "nomo-baseline"
            )
            reference["source_files"]["nomo"] = [
                {
                    "path": str(
                        (
                            self.suite_root / nomo_source["path"]
                        ).resolve()
                    ),
                    "sha256": nomo_source["sha256"],
                },
                {
                    "path": str(
                        (
                            self.suite_root
                            / nomo_source["project_manifest"]
                        ).resolve()
                    ),
                    "sha256": nomo_source["project_manifest_sha256"],
                },
            ]
            reference["binaries"]["nomo-baseline"] = baseline_binary
            reference["commands"]["nomo_baseline_emit_c"] = (
                self.command_record(
                    [
                        self.fixture_path("tools", "nomo"),
                        "build",
                        str(project.resolve()),
                        "--emit-c",
                    ]
                )
            )
            link_flags = (
                ["-lm"]
                if workload.get("link_math")
                else []
            )
            reference["commands"]["nomo_baseline_clang"] = (
                self.command_record(
                    [
                        self.fixture_path("tools", "clang"),
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        *benchmark.BASE_C_FLAGS,
                        generated_path,
                        "-o",
                        baseline_binary["path"],
                        *link_flags,
                    ]
                )
            )
            generated_c = {
                "path": generated_path,
                "sha256": "7" * 64,
                "unmodified_after_emit": True,
                "decisional_release_lane": False,
            }
        return {
            "kind": "reference-and-correctness-baseline",
            "source_files": reference["source_files"],
            "compiled_sources": reference["compiled_sources"],
            "commands": {
                name: {
                    **record,
                    "cwd": str(self.suite_root.parent.parent.resolve()),
                    "duration_ns": 1,
                    "exit_code": 0,
                }
                for name, record in reference["commands"].items()
            },
            "compiler_output": {
                **{
                    lane: {"stdout": "", "stderr": ""}
                    for lane in benchmark.REFERENCE_LANES
                },
                **(
                    {
                        "nomo-baseline": {
                            "emit_stdout": "",
                            "emit_stderr": "",
                            "clang_stdout": "",
                            "clang_stderr": "",
                        }
                    }
                    if include_nomo_baseline
                    else {}
                ),
            },
            "generated_c": generated_c,
            "binaries": reference["binaries"],
            "compile_time_excluded_from_run_time": True,
        }

    def available_release_lane(self, lane: str) -> dict:
        commit = ("a" if lane == "candidate" else "b") * 40
        binary_sha = ("c" if lane == "candidate" else "d") * 64
        nomo_path = self.fixture_path(lane, "target", "release", "nomo")
        if self.__class__._rust_toolchain_fixture is None:
            cargo_invocation = benchmark.resolve_executable(
                "cargo", "Cargo fixture"
            )
            rustup = benchmark.resolve_executable(
                "rustup", "rustup fixture"
            ).resolve()
            proxy_rustc = benchmark.resolve_executable(
                "rustc", "rustc fixture"
            )
            rustc_sysroot = subprocess.run(
                [str(proxy_rustc), "--print", "sysroot"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            suffix = ".exe" if os.name == "nt" else ""
            cargo = Path(rustc_sysroot) / "bin" / f"cargo{suffix}"
            rustc = Path(rustc_sysroot) / "bin" / f"rustc{suffix}"
            self.__class__._rust_toolchain_fixture = {
                "cargo_invocation": cargo_invocation,
                "rustup": rustup,
                "cargo": cargo,
                "rustc": rustc,
                "rustc_version": subprocess.run(
                    [str(rustc), "-vV"],
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip(),
                "rustc_sysroot": subprocess.run(
                    [str(rustc), "--print", "sysroot"],
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip(),
                "cargo_version": subprocess.run(
                    [str(cargo), "--version"],
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip(),
            }
        rust_toolchain = self.__class__._rust_toolchain_fixture
        assert rust_toolchain is not None
        cargo_invocation = rust_toolchain["cargo_invocation"]
        rustup = rust_toolchain["rustup"]
        cargo = rust_toolchain["cargo"]
        rustc = rust_toolchain["rustc"]
        rustc_version = rust_toolchain["rustc_version"]
        rustc_sysroot = rust_toolchain["rustc_sysroot"]
        cargo_version = rust_toolchain["cargo_version"]
        cargo_home = str(
            Path(self.fixture_path(lane, f"{lane}-cargo-home")).resolve()
        )
        cargo_environment = {
            "CARGO_TARGET_DIR": str(
                Path(self.fixture_path(lane, "target")).resolve()
            ),
            "CARGO_HOME": cargo_home,
            "RUSTC": str(rustc),
        }
        checkout = self.fixture_path(lane)
        resolution_environment = {
            "CARGO_TARGET_DIR": cargo_environment["CARGO_TARGET_DIR"],
            "CARGO_HOME": cargo_environment["CARGO_HOME"],
        }
        capability = {
            "label": lane,
            "status": "available",
            "reason": "fixture",
            "help_command": self.full_command(
                [nomo_path, "build", "--help"], cwd=checkout
            ),
            "nomo_path": nomo_path,
            "nomo_sha256": binary_sha,
        }
        return {
            "label": lane,
            "status": "available",
            "reason": "fixture",
            "emit_c_fallback_used": False,
            "checkout": checkout,
            "expected_commit": commit,
            "detached_head": True,
            "repository": {"commit": commit},
            "origin_url": "git@github.com:nomo-lang/nomo.git",
            "normalized_origin": "github.com/nomo-lang/nomo",
            "nomo_path": nomo_path,
            "nomo_sha256": binary_sha,
            "compiler_build": {
                "repository_before": {"commit": commit},
                "repository_after": {"commit": commit},
                "expected_commit": commit,
                "detached_head": True,
                "origin_main_commit": commit if lane == "main" else None,
                "remote_main_commit": commit if lane == "main" else None,
                "origin_url": "git@github.com:nomo-lang/nomo.git",
                "normalized_origin": "github.com/nomo-lang/nomo",
                "command": self.full_command(
                    [str(cargo), "build", "--locked", "--release", "--bin", "nomo"],
                    cwd=checkout,
                    approved_environment_overrides=cargo_environment,
                ),
                "environment": cargo_environment,
                "cargo_configs": [],
                "rustup_resolution": {
                    "kind": "rustup-which-v1",
                    "invocation_path": str(cargo_invocation),
                    "rustup": {
                        "path": str(rustup),
                        "realpath": str(rustup.resolve()),
                        "sha256": v1.sha256_file(rustup.resolve()),
                    },
                    "cargo_command": self.full_command(
                        [str(rustup), "which", "cargo"],
                        cwd=checkout,
                        approved_environment_overrides=resolution_environment,
                    ),
                    "rustc_command": self.full_command(
                        [str(rustup), "which", "rustc"],
                        cwd=checkout,
                        approved_environment_overrides=resolution_environment,
                    ),
                    "selected_sysroot": rustc_sysroot,
                },
                "cargo": {
                    "path": str(cargo),
                    "realpath": str(cargo.resolve()),
                    "sha256": v1.sha256_file(cargo.resolve()),
                    "version_output": cargo_version,
                    "version_command": self.full_command(
                        [str(cargo), "--version"],
                        cwd=checkout,
                        approved_environment_overrides=cargo_environment,
                    ),
                },
                "rustc": {
                    "path": str(rustc),
                    "realpath": str(rustc.resolve()),
                    "sha256": v1.sha256_file(rustc.resolve()),
                    "version_output": rustc_version,
                    "version_fields": benchmark.parse_rustc_verbose_version(
                        rustc_version
                    ),
                    "version_command": self.full_command(
                        [str(rustc), "-vV"],
                        cwd=checkout,
                        approved_environment_overrides=cargo_environment,
                    ),
                    "sysroot": rustc_sysroot,
                    "sysroot_command": self.full_command(
                        [str(rustc), "--print", "sysroot"],
                        cwd=checkout,
                        approved_environment_overrides=cargo_environment,
                    ),
                    "toolchain": Path(rustc_sysroot).name,
                    "driver_files": [
                        {
                            "path": str(path.resolve()),
                            "sha256": v1.sha256_file(path),
                        }
                        for path in sorted(
                            (
                                path
                                for path in Path(rustc_sysroot).rglob(
                                    "*rustc_driver*"
                                )
                                if path.is_file()
                            ),
                            key=lambda path: str(path.resolve()).casefold(),
                        )
                    ],
                },
                "binary": {"path": nomo_path, "sha256": binary_sha},
                "stdout": "",
                "stderr": "",
            },
            "capabilities": {
                "release": capability,
                "emit-c": capability,
            },
        }

    def correctness_only_result(self) -> dict:
        host = {"os": "Linux", "fixture": True}
        toolchains = {
            "build_environment": benchmark.sanitized_build_environment()[1],
            "runtime_environments": {
                "default": benchmark._environment({})[1],
                "go": benchmark._environment({"GOMAXPROCS": "1"})[1],
            },
            "nomo": {
                "path": self.fixture_path("tools", "nomo"),
                "sha256": "7" * 64,
            },
            "clang": {
                "path": self.fixture_path("tools", "clang"),
                "realpath": self.fixture_path("tools", "clang"),
                "sha256": "9" * 64,
                "version": "21.0.0",
                "version_output": "Apple clang version 21.0.0",
                "installation": self.fixture_path("tools"),
                "target_triple": "x86_64-unknown-linux-gnu",
                "driver_config_flags": list(
                    benchmark.CLANG_DRIVER_CONFIG_FLAGS
                ),
                "target_command": self.full_command(
                    [
                        self.fixture_path("tools", "clang"),
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        "-print-target-triple",
                    ]
                ),
            },
            "clangxx": {
                "path": self.fixture_path("tools", "clang++"),
                "realpath": self.fixture_path("tools", "clang++"),
                "sha256": "8" * 64,
                "version": "21.0.0",
                "version_output": "Apple clang version 21.0.0",
                "installation": self.fixture_path("tools"),
                "target_triple": "x86_64-unknown-linux-gnu",
                "driver_config_flags": list(
                    benchmark.CLANG_DRIVER_CONFIG_FLAGS
                ),
                "target_command": self.full_command(
                    [
                        self.fixture_path("tools", "clang++"),
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        "-print-target-triple",
                    ]
                ),
            },
            "go": {"path": self.fixture_path("tools", "go")},
        }
        release_lanes = {
            lane: {
                "label": lane,
                "status": "unavailable",
                "reason": "correctness only",
                "emit_c_fallback_used": False,
            }
            for lane in ("candidate", "main")
        }
        source_lock = benchmark.frozen_source_lock(self.manifest)
        authorization = benchmark.environment_qualification(
            self.manifest,
            None,
            benchmark.qualification_bindings(
                host, toolchains, source_lock, release_lanes
            ),
        )
        result = {
            "schema": 2,
            "suite": "nomo-benchmarksgame-cpu-parity-v2",
            "manifest_version": "2026-07-28",
            "mode": "correctness",
            "status": "correctness-only",
            "created_at_utc": "2026-07-28T00:00:00+00:00",
            "claims": {
                "claim_eligible": False,
                "scope": "RFC 0043 frozen three-workload CPU parity suite only",
                "limitations": ["a", "b"],
            },
            "provenance": {
                "repository": {},
                "manifest_path": str(self.manifest_path),
                "manifest_sha256": benchmark.EXPECTED_V2_MANIFEST_SHA,
                "predecessor": {},
                "rfc": {},
                "host": host,
                "toolchains": toolchains,
                "collector": benchmark.collector_descriptor_for_host("Linux"),
                "methodology": self.manifest["methodology"],
                "thresholds": self.manifest["thresholds"],
                "environment_qualification": authorization,
                "source_lock": source_lock,
                "prepared_bundle_sha256": None,
                "prepared_bundle_path": None,
                "qualification_request_path": None,
            },
            "release_lanes": release_lanes,
            "builds": {
                workload: {
                    "references": self.schema_reference_build(
                        workload, include_nomo_baseline=True
                    ),
                    "modes": {},
                }
                for workload in benchmark.WORKLOAD_IDS
            },
            "build_failures": [],
            "correctness": [
                self.correctness_item(workload, "baseline-emit-c")
                for workload in benchmark.WORKLOAD_IDS
            ],
            "protocols": {
                build_mode: {
                    "build_mode": build_mode,
                    "status": "unavailable",
                    "reason": "correctness only",
                    "correctness": [],
                    "batches": [],
                    "verdict": "not_evaluated",
                }
                for build_mode in benchmark.FORMAL_BUILD_MODES
            },
            "overall_verdict": "not_evaluated",
        }
        result = self.project_windows_paths_as_linux_artifact(result)
        self.bind_synthetic_build_environment(result)
        self.rebind_release_metadata(result)
        self.bind_synthetic_runtime_environments(result)
        result["provenance"]["environment_qualification"] = (
            benchmark.environment_qualification(
                self.manifest,
                None,
                benchmark.qualification_bindings(
                    result["provenance"]["host"],
                    result["provenance"]["toolchains"],
                    result["provenance"]["source_lock"],
                    result["release_lanes"],
                ),
            )
        )
        return result

    def completed_result(self) -> dict:
        result = self.correctness_only_result()
        result.update(
            {
                "mode": "measure",
                "status": "completed",
                "claims": {
                    **result["claims"],
                    "claim_eligible": True,
                },
                "release_lanes": {
                    lane: self.available_release_lane(lane)
                    for lane in ("candidate", "main")
                },
                "builds": {
                    workload: {
                        "references": self.schema_reference_build(workload),
                        "modes": {
                            build_mode: {
                                lane: self.formal_build(
                                    lane, build_mode, workload
                                )
                                for lane in ("candidate", "main")
                            }
                            for build_mode in benchmark.FORMAL_BUILD_MODES
                        },
                    }
                    for workload in benchmark.WORKLOAD_IDS
                },
                "correctness": [],
                "protocols": {
                    build_mode: self.completed_protocol(build_mode)
                    for build_mode in benchmark.FORMAL_BUILD_MODES
                },
                "overall_verdict": "pass",
            }
        )
        result["provenance"]["manifest_path"] = str(self.manifest_path)
        result["provenance"]["manifest_sha256"] = benchmark.EXPECTED_V2_MANIFEST_SHA
        result["provenance"]["prepared_bundle_sha256"] = "e" * 64
        result = self.project_windows_paths_as_linux_artifact(result)
        self.bind_synthetic_build_environment(result)
        self.rebind_release_metadata(result)
        self.bind_synthetic_runtime_environments(result)
        bindings = benchmark.qualification_bindings(
            result["provenance"]["host"],
            result["provenance"]["toolchains"],
            result["provenance"]["source_lock"],
            result["release_lanes"],
            result["provenance"]["prepared_bundle_sha256"],
        )
        checks = {
            check: self.qualified_check(f"qualified:{check}")
            for check in benchmark.EXPECTED_REQUIRED_CHECKS
        }
        checks["canonical_host_identity"] = self.qualified_check(
            bindings["authority_host_sha256"]
        )
        checks["toolchain_identity"] = self.qualified_check(
            bindings["reference_toolchains_sha256"]
        )
        checks["frozen_source_lock"] = self.qualified_check(
            bindings["frozen_source_lock_sha256"]
        )
        document = {
            "schema": 1,
            "canonical_host_id": "fixture-host",
            "captured_at_utc": "2026-07-28T00:00:00+00:00",
            "dynamic_policy": benchmark.DYNAMIC_ENVIRONMENT_POLICY,
            "bindings": bindings,
            "checks": checks,
        }
        qualification_path = Path(self.temporary.name) / "environment.json"
        benchmark.write_canonical_json(qualification_path, document)
        result["provenance"]["environment_qualification"] = (
            benchmark.environment_qualification(
                self.manifest, str(qualification_path), bindings
            )
        )
        authorization_sha = result["provenance"]["environment_qualification"][
            "qualification_sha256"
        ]
        snapshot_factory = self.dynamic_snapshot_factory()
        for protocol in result["protocols"].values():
            for batch in protocol["batches"]:
                batch["static_authorization_sha256"] = authorization_sha
                batch["dynamic_environment_before"] = snapshot_factory(
                    bindings["authority_host_sha256"]
                )
                batch["dynamic_environment_after"] = snapshot_factory(
                    bindings["authority_host_sha256"]
                )
        return result

    def prepared_bundle_fixture(self) -> tuple[dict, Path]:
        root = Path(self.temporary.name) / f"prepared-{time.time_ns()}"
        root.mkdir()
        result = self.project_result_to_producer_os(
            self.completed_result(), platform.system()
        )
        result["mode"] = "prepare"
        result["status"] = "unavailable"
        result["claims"]["claim_eligible"] = False
        result["correctness"] = []
        result["protocols"] = {
            build_mode: {
                "build_mode": build_mode,
                "status": "unavailable",
                "reason": "prepared but not measured",
                "correctness": [],
                "batches": [],
                "verdict": "not_evaluated",
            }
            for build_mode in benchmark.FORMAL_BUILD_MODES
        }
        result["overall_verdict"] = "not_evaluated"
        result["provenance"]["toolchains"]["build_environment"] = (
            benchmark.sanitized_build_environment()[1]
        )
        self.bind_build_command_environments(
            result,
            result["provenance"]["toolchains"]["build_environment"],
        )
        result["provenance"]["toolchains"]["runtime_environments"] = {
            "default": benchmark._environment({})[1],
            "go": benchmark._environment({"GOMAXPROCS": "1"})[1],
        }
        if os.name == "nt":
            result["provenance"]["host"] = {
                "os": "Windows",
                "architecture": platform.machine(),
                "fixture": True,
            }
            result["provenance"]["manifest_path"] = str(
                self.manifest_path.resolve()
            )
            result["provenance"]["collector"] = (
                benchmark.collector_descriptor_for_host("Windows")
            )
            for workload in self.manifest["workloads"]:
                references = result["builds"][workload["id"]]["references"]
                for lane in benchmark.REFERENCE_LANES:
                    source = workload["sources"][lane]
                    references["source_files"][lane] = {
                        "path": str(
                            (
                                self.suite_root / source["path"]
                            ).resolve()
                        ),
                        "sha256": source["sha256"],
                    }

        def write_file(path: Path, content: bytes) -> dict:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            return {"path": str(path.resolve()), "sha256": v1.sha256_file(path)}

        for lane in ("candidate", "main"):
            state = result["release_lanes"][lane]
            if os.name == "nt":
                checkout = root / "checkouts" / lane
                checkout.mkdir(parents=True)
                state["checkout"] = str(checkout.resolve())
                state["compiler_build"]["repository_before"] = state[
                    "repository"
                ]
                state["compiler_build"]["repository_after"] = state[
                    "repository"
                ]
            target_dir = root / "compiler-build" / lane
            cargo_home = (
                root / "compiler-build" / f"{lane}-cargo-home"
            )
            compiler = write_file(
                benchmark.binary_path(target_dir / "release", "nomo"),
                f"{lane}-compiler".encode(),
            )
            state["nomo_path"] = compiler["path"]
            state["nomo_sha256"] = compiler["sha256"]
            compiler_build = state["compiler_build"]
            compiler_build["binary"] = compiler
            rustc_path = compiler_build["rustc"]["path"]
            compiler_build["environment"] = {
                "CARGO_TARGET_DIR": str(target_dir.resolve()),
                "CARGO_HOME": str(cargo_home.resolve()),
                "RUSTC": rustc_path,
            }
            compiler_build["cargo_configs"] = []
            resolution_environment = {
                "CARGO_TARGET_DIR": str(target_dir.resolve()),
                "CARGO_HOME": str(cargo_home.resolve()),
            }
            rustup_resolution = compiler_build["rustup_resolution"]
            rustup_path = rustup_resolution["rustup"]["path"]
            rustup_resolution["cargo_command"] = self.full_command(
                [rustup_path, "which", "cargo"],
                cwd=state["checkout"],
                approved_environment_overrides=resolution_environment,
            )
            rustup_resolution["rustc_command"] = self.full_command(
                [rustup_path, "which", "rustc"],
                cwd=state["checkout"],
                approved_environment_overrides=resolution_environment,
            )
            compiler_build["command"] = self.full_command(
                [
                    compiler_build["cargo"]["path"],
                    "build",
                    "--locked",
                    "--release",
                    "--bin",
                    "nomo",
                ],
                cwd=state["checkout"],
                approved_environment_overrides={
                    "CARGO_TARGET_DIR": str(target_dir.resolve()),
                    "CARGO_HOME": str(cargo_home.resolve()),
                    "RUSTC": rustc_path,
                },
            )
            compiler_build["cargo"]["version_command"] = self.full_command(
                [compiler_build["cargo"]["path"], "--version"],
                cwd=state["checkout"],
                approved_environment_overrides=compiler_build["environment"],
            )
            compiler_build["rustc"]["version_command"] = self.full_command(
                [rustc_path, "-vV"],
                cwd=state["checkout"],
                approved_environment_overrides=compiler_build["environment"],
            )
            compiler_build["rustc"]["sysroot_command"] = self.full_command(
                [rustc_path, "--print", "sysroot"],
                cwd=state["checkout"],
                approved_environment_overrides=compiler_build["environment"],
            )
            for build_mode in benchmark.FORMAL_BUILD_MODES:
                capability = state["capabilities"][build_mode]
                capability["nomo_path"] = compiler["path"]
                capability["nomo_sha256"] = compiler["sha256"]
                capability["help_command"] = self.full_command(
                    [compiler["path"], "build", "--help"],
                    cwd=state["checkout"],
                )

        toolchains = result["provenance"]["toolchains"]
        for workload in self.manifest["workloads"]:
            workload_id = workload["id"]
            build = result["builds"][workload_id]
            references = build["references"]
            copied_sources = {}
            reference_binaries = {}
            for lane in benchmark.REFERENCE_LANES:
                original = (
                    self.suite_root / workload["sources"][lane]["path"]
                )
                suffix = original.suffix
                copied = root / "references" / workload_id / f"{lane}{suffix}"
                copied.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, copied)
                copied_sources[lane] = {
                    "path": str(copied.resolve()),
                    "sha256": v1.sha256_file(copied),
                }
                reference_binaries[lane] = write_file(
                    benchmark.binary_path(
                        root / "reference-bin",
                        f"{workload_id}-{lane}",
                    ),
                    f"{workload_id}-{lane}".encode(),
                )
            references["compiled_sources"] = copied_sources
            references["binaries"] = reference_binaries
            references["generated_c"] = None
            link_flags = (
                ["-lm"]
                if workload.get("link_math")
                and result["provenance"]["host"]["os"] != "Windows"
                else []
            )
            reference_cwd = str(self.suite_root.parent.parent.resolve())
            references["commands"] = {
                "c_build": self.full_command(
                    [
                        toolchains["clang"]["path"],
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        *benchmark.BASE_C_FLAGS,
                        copied_sources["c"]["path"],
                        "-o",
                        reference_binaries["c"]["path"],
                        *link_flags,
                    ],
                    cwd=reference_cwd,
                ),
                "cpp_build": self.full_command(
                    [
                        toolchains["clangxx"]["path"],
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        *benchmark.BASE_CPP_FLAGS,
                        copied_sources["cpp"]["path"],
                        "-o",
                        reference_binaries["cpp"]["path"],
                        *link_flags,
                    ],
                    cwd=reference_cwd,
                ),
                "semantic-c_build": self.full_command(
                    [
                        toolchains["clang"]["path"],
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        *benchmark.BASE_C_FLAGS,
                        copied_sources["semantic-c"]["path"],
                        "-o",
                        reference_binaries["semantic-c"]["path"],
                        *link_flags,
                    ],
                    cwd=reference_cwd,
                ),
                "go_build": self.full_command(
                    [
                        toolchains["go"]["path"],
                        "build",
                        "-o",
                        reference_binaries["go"]["path"],
                        copied_sources["go"]["path"],
                    ],
                    cwd=reference_cwd,
                    approved_environment_overrides={
                        key: str(
                            (
                                Path(copied_sources["go"]["path"]).parent
                                / directory
                            ).resolve()
                        )
                        for key, directory in benchmark.GO_BUILD_CACHE_DIRECTORIES.items()
                    },
                ),
            }
            for build_mode in benchmark.FORMAL_BUILD_MODES:
                for lane in ("candidate", "main"):
                    formal = build["modes"][build_mode][lane]
                    state = result["release_lanes"][lane]
                    project = (
                        root
                        / "build"
                        / workload_id
                        / build_mode
                        / lane
                        / "project"
                    )
                    nomo_source = (
                        self.suite_root / workload["sources"]["nomo"]["path"]
                    )
                    nomo_manifest = (
                        self.suite_root
                        / workload["sources"]["nomo"]["project_manifest"]
                    )
                    copied_source_path = project / "src" / "main.nomo"
                    copied_manifest_path = project / "nomo.toml"
                    copied_source_path.parent.mkdir(
                        parents=True, exist_ok=True
                    )
                    shutil.copy2(nomo_source, copied_source_path)
                    shutil.copy2(nomo_manifest, copied_manifest_path)
                    generated = write_file(
                        project / "build" / "c" / "main.c",
                        f"{workload_id}-{build_mode}-{lane}-c".encode(),
                    )
                    project_name = benchmark.parse_project_name(
                        nomo_manifest
                    )
                    binary = write_file(
                        benchmark.binary_path(
                            project / "build" / "bin", project_name
                        ),
                        f"{workload_id}-{build_mode}-{lane}-binary".encode(),
                    )
                    formal["repository"] = state["repository"]
                    formal["nomo"] = {
                        "path": state["nomo_path"],
                        "sha256": state["nomo_sha256"],
                    }
                    formal["source"] = {
                        "path": str(nomo_source.resolve()),
                        "sha256": v1.sha256_file(nomo_source),
                    }
                    formal["project"] = {
                        "path": str(project.resolve()),
                        "source_relative_path": workload["sources"]["nomo"][
                            "path"
                        ],
                        "source": formal["source"],
                        "project_manifest_relative_path": workload["sources"][
                            "nomo"
                        ]["project_manifest"],
                        "project_manifest": {
                            "path": str(nomo_manifest.resolve()),
                            "sha256": v1.sha256_file(nomo_manifest),
                        },
                        "copied_source": {
                            "path": str(copied_source_path.resolve()),
                            "sha256": v1.sha256_file(copied_source_path),
                        },
                        "copied_project_manifest": {
                            "path": str(copied_manifest_path.resolve()),
                            "sha256": v1.sha256_file(copied_manifest_path),
                        },
                        "compiler_checkout": state["checkout"],
                        "compiler_commit": state["expected_commit"],
                    }
                    formal["generated_c"] = {
                        **generated,
                        (
                            "unmodified_after_build"
                            if build_mode == "release"
                            else "unmodified_after_emit"
                        ): True,
                    }
                    formal["binary"] = binary
                    if build_mode == "release":
                        formal["command"] = self.full_command(
                            [
                                state["nomo_path"],
                                "build",
                                str(project.resolve()),
                                "--release",
                            ],
                            cwd=state["checkout"],
                        )
                        object_file = write_file(
                            project / "build" / "c" / "main.o",
                            f"{workload_id}-{lane}-object".encode(),
                        )
                        compiler = {
                            key: toolchains["clang"][key]
                            for key in (
                                "path",
                                "realpath",
                                "sha256",
                                "version_output",
                                "target_triple",
                            )
                        }
                        backend = {
                            "schema": 1,
                            "complete_argv": True,
                            "compiler": compiler,
                            "objects": [object_file],
                            "compile_commands": [
                                self.full_command(
                                    [
                                        compiler["path"],
                                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                                        *benchmark.BASE_C_FLAGS,
                                        "-c",
                                        generated["path"],
                                        "-o",
                                        object_file["path"],
                                    ],
                                    cwd=state["checkout"],
                                )
                            ],
                            "link_command": self.full_command(
                                [
                                    compiler["path"],
                                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                                    object_file["path"],
                                    "-o",
                                    binary["path"],
                                    *link_flags,
                                ],
                                cwd=state["checkout"],
                            ),
                            "generated_c": generated,
                            "binary": binary,
                        }
                        provenance_path = (
                            project / "build" / "release-provenance.json"
                        )
                        v1.write_result(provenance_path, backend)
                        formal["backend_provenance"] = backend
                        formal["backend_provenance_path"] = str(
                            provenance_path.resolve()
                        )
                        formal["backend_provenance_sha256"] = v1.sha256_file(
                            provenance_path
                        )
                        metadata = self.release_build_metadata(
                            formal,
                            producer_size_bytes=Path(
                                state["nomo_path"]
                            ).stat().st_size,
                        )
                        metadata_path = (
                            project / "build" / "nomo-build-metadata.json"
                        )
                        metadata_path.write_bytes(
                            self.build_metadata_bytes(metadata)
                        )
                        formal["build_metadata"] = metadata
                        formal["build_metadata_path"] = str(
                            metadata_path.resolve()
                        )
                        formal["build_metadata_sha256"] = v1.sha256_file(
                            metadata_path
                        )
                    else:
                        formal["emit_command"] = self.full_command(
                            [
                                state["nomo_path"],
                                "build",
                                str(project.resolve()),
                                "--emit-c",
                            ],
                            cwd=state["checkout"],
                        )
                        formal["clang_command"] = self.full_command(
                            [
                                toolchains["clang"]["path"],
                                *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                                *benchmark.BASE_C_FLAGS,
                                generated["path"],
                                "-o",
                                binary["path"],
                                *link_flags,
                            ],
                            cwd=state["checkout"],
                        )
        return benchmark.write_prepared_bundle(
            result, root, self.manifest
        ), root


if __name__ == "__main__":
    unittest.main()
