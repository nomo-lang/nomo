from __future__ import annotations

import copy
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
            {"GOENV": "off"},
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
        benchmark.validate_protocol(
            protocol,
            "release",
            self.manifest,
            result["builds"],
            result["provenance"]["collector"]["id"],
            authorization,
            "Linux",
            "x86_64",
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
            )

    def test_dynamic_snapshot_and_pair_issues_are_recomputed(self) -> None:
        result = self.completed_result()
        protocol = copy.deepcopy(result["protocols"]["release"])
        authorization = result["provenance"]["environment_qualification"]
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
            shadow = f"/usr/local/bin/{Path(trusted).name}"
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

    def test_windows_powercfg_path_comes_from_system_api_not_environment(
        self,
    ) -> None:
        trusted = Path(self.temporary.name) / "real-system32"
        trusted.mkdir()
        with mock.patch.object(
            benchmark,
            "windows_system_directory",
            return_value=trusted,
        ), mock.patch.dict(
            os.environ,
            {
                "SystemRoot": str(Path(self.temporary.name) / "shadow-root"),
                "WINDIR": str(Path(self.temporary.name) / "shadow-windir"),
            },
        ):
            expected = benchmark.expected_dynamic_system_path(
                "Windows", "powercfg"
            )
        self.assertEqual(expected, str(trusted / "powercfg.exe"))
        self.assertNotIn("shadow", expected)

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
            },
        ):
            benchmark.canonical_windows_build_support(refresh=True)
            actual, projection = benchmark.sanitized_build_environment()
            self.assertNotEqual(actual["INCLUDE"], poison)
            self.assertNotEqual(actual["LIB"], poison)
            self.assertNotEqual(actual["LIBPATH"], poison)
            self.assertNotIn("CFLAGS", actual)
            self.assertIn("windows_toolchain", projection)
            source = Path(self.temporary.name) / "sdk-probe.c"
            binary = Path(self.temporary.name) / "sdk-probe.exe"
            source.write_text(
                "#include <ctype.h>\nint main(void) { return isdigit('1') ? 0 : 1; }\n",
                encoding="utf-8",
            )
            clang = benchmark.resolve_executable("clang", "Clang C")
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
        self.assertTrue(binary.is_file())

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
        cargo = benchmark.resolve_executable("cargo", "Cargo")
        rustc = benchmark.rustc_for_cargo(cargo)
        self.assertEqual(cargo.name.lower().removesuffix(".exe"), "cargo")
        self.assertEqual(rustc.name.lower().removesuffix(".exe"), "rustc")
        if cargo.resolve() == rustc.resolve():
            self.assertNotEqual(cargo, rustc)
        root = Path(self.temporary.name)
        cargo_environment = {
            "CARGO_TARGET_DIR": str((root / "target").resolve()),
            "CARGO_HOME": str((root / "cargo-home").resolve()),
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
            probe = subprocess.run(
                ["/usr/bin/env", "-0"],
                check=True,
                capture_output=True,
                env=actual,
            )
            sample, stdout = collector.run(
                ["/usr/bin/env", "-0"],
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
        normalized = benchmark._validate_process_output(
            ["/tmp/program"], 0, b"ok\r\n", b"", b"ok\n"
        )
        self.assertEqual(normalized, b"ok\n")
        with self.assertRaisesRegex(benchmark.HarnessError, "output mismatch"):
            benchmark._validate_process_output(
                ["/tmp/program"], 0, b"ok \r\n", b"", b"ok\n"
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
        with mock.patch.object(benchmark.os, "name", "nt"):
            self.assertEqual(benchmark.math_flags(workload), [])
        with mock.patch.object(benchmark.os, "name", "posix"):
            self.assertEqual(benchmark.math_flags(workload), ["-lm"])

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
            v1,
            "run_capture",
            return_value=(record, b"Usage: nomo build [--emit-c]\n", b""),
        ):
            result = benchmark.release_capability(executable, "candidate")
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(
            result["help_command"]["argv"],
            [str(executable), "build", "--help"],
        )
        with mock.patch.object(
            v1,
            "run_capture",
            return_value=(
                record,
                b"Usage: nomo build [OPTIONS]\n  --release  optimized\n",
                b"",
            ),
        ):
            result = benchmark.release_capability(executable, "candidate")
        self.assertEqual(result["status"], "available", result)

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
            v1,
            "run_capture",
            return_value=(record, b"Usage: nomo build [--emit-c]\n", b""),
        ):
            result = benchmark.emit_c_capability(executable, "candidate")
        self.assertEqual(result["status"], "available")

    def test_missing_release_mode_is_unavailable_before_environment_gate(self) -> None:
        arguments = Namespace(
            candidate_commit="a" * 40,
            main_commit="b" * 40,
            candidate_checkout="/tmp/candidate",
            main_checkout="/tmp/main",
            cargo="cargo",
            environment_qualification=None,
            prepared_bundle=None,
        )
        states = []
        for lane, checkout, commit in (
            ("candidate", "/tmp/candidate", "a" * 40),
            ("main", "/tmp/main", "b" * 40),
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

            def run_capture(
                command: list[str],
                timeout_seconds: float,
                cwd: Path,
                environment: dict[str, str],
            ) -> tuple[dict, bytes, bytes]:
                target = Path(environment["CARGO_TARGET_DIR"])
                cargo_home = Path(environment["CARGO_HOME"])
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
                    stdout = b"/tmp/fixture-rust-toolchain\n"
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
                v1, "tool_version", return_value="cargo 1.99.0"
            ), mock.patch.object(
                v1, "run_capture", side_effect=run_capture
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
            '[build]\nrustc = "/tmp/unbound-rustc"\n',
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
                v1, "git_capture", return_value="/tmp/fake-origin"
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
            candidate_checkout="/tmp/candidate",
            main_checkout="/tmp/main",
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
            v1.write_result(request_path, changed)
            with self.subTest(request_field=field):
                with self.assertRaisesRegex(
                    benchmark.HarnessError, "canonical request"
                ):
                    benchmark.validate_prepared_bundle_authority(
                        result, bundle, require_exact_result=True
                    )
            v1.write_result(request_path, canonical_request)

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
            v1.write_result(metadata_path, changed)
            with self.assertRaises(benchmark.HarnessError):
                benchmark.load_prepared_bundle(bundle)
        v1.write_result(metadata_path, canonical_metadata)
        self.assertEqual(benchmark.load_prepared_bundle(bundle), result)

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
        v1.write_result(metadata_path, metadata)
        v1.write_result(
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
                benchmark.HarnessError, "build environment"
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
        arguments = benchmark.parse_arguments(["--mode", "measure"])
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
        changed["builds"]["spectral-norm"]["modes"]["release"]["candidate"][
            "command"
        ] = self.command_record(
            ["/tmp/nomo", "build", "/tmp/project", "--emit-c"]
        )
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
        self.assertEqual(completed.stdout, fixture)
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
        result = self.completed_result()
        benchmark.validate_result_schema(result, self.result_schema_path)
        with mock.patch.object(benchmark, "validate_result_prepared_authority"):
            benchmark.validate_result(result, self.manifest)

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
        sample["command_argv"] = ["/usr/bin/true"]
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
            "#!/usr/bin/env python3\nprint('wrong')\n",
            encoding="utf-8",
        )
        mismatch.chmod(0o755)
        programs["output-mismatch"] = mismatch
        timeout = root / "correctness-timeout"
        timeout.write_text(
            "#!/usr/bin/env python3\nimport time\ntime.sleep(10)\n",
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
                self.bind_formal_candidate_binary(
                    result, "spectral-norm", "release", executable
                )
                binaries = {
                    workload_id: {}
                    for workload_id in benchmark.WORKLOAD_IDS
                }
                binaries["spectral-norm"]["candidate"] = executable
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
        root = Path(self.temporary.name)
        timeout_program = root / "timeout-program"
        timeout_program.write_text(
            "#!/usr/bin/env python3\nimport time\ntime.sleep(10)\n",
            encoding="utf-8",
        )
        timeout_program.chmod(0o755)
        mismatch_program = root / "mismatch-program"
        mismatch_program.write_text(
            "#!/usr/bin/env python3\nprint('wrong')\n",
            encoding="utf-8",
        )
        mismatch_program.chmod(0o755)
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
            self.bind_formal_candidate_binary(
                result, failed_workload, build_mode, executable
            )
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
            "path": "/tmp/replacement.cpp",
            "sha256": "0" * 64,
        }
        reference["compiled_sources"]["cpp"] = {
            "path": "/tmp/replacement-copy.cpp",
            "sha256": "0" * 64,
        }
        command = reference["commands"]["cpp_build"]
        command["argv"][
            len(benchmark.CLANG_DRIVER_CONFIG_FLAGS)
            + len(benchmark.BASE_CPP_FLAGS)
            + 1
        ] = "/tmp/replacement-copy.cpp"
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
                backend["compiler"]["path"] = "/usr/bin/gcc"
                backend["compile_commands"][0]["argv"][0] = "/usr/bin/gcc"
                backend["link_command"]["argv"][0] = "/usr/bin/gcc"
            elif mutation == "target":
                backend["compiler"]["target_triple"] = "x86_64-unknown-linux-gnu"
            else:
                backend["compile_commands"][0]["argv"].remove("-O3")
            for record in [
                *backend["compile_commands"],
                backend["link_command"],
            ]:
                record["command"] = v1.command_text(record["argv"])
            with self.assertRaises(benchmark.HarnessError):
                benchmark.validate_build_provenance(result, self.manifest)

    def test_windows_exact_argv_does_not_expect_libm(self) -> None:
        result = self.completed_result()
        result["provenance"]["host"]["os"] = "Windows"
        for workload_id in ("spectral-norm", "n-body"):
            build = result["builds"][workload_id]
            for name in ("c_build", "cpp_build", "semantic-c_build"):
                record = build["references"]["commands"][name]
                record["argv"].remove("-lm")
                record["command"] = v1.command_text(record["argv"])
            for mode in benchmark.FORMAL_BUILD_MODES:
                for lane in ("candidate", "main"):
                    formal = build["modes"][mode][lane]
                    if mode == "emit-c":
                        records = [formal["clang_command"]]
                    else:
                        records = [formal["backend_provenance"]["link_command"]]
                    for record in records:
                        record["argv"].remove("-lm")
                        record["command"] = v1.command_text(record["argv"])
        benchmark.validate_build_provenance(result, self.manifest)

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
        binary = {
            "path": str(executable.resolve()),
            "sha256": v1.sha256_file(executable),
        }
        formal = result["builds"][workload_id]["modes"][build_mode][
            "candidate"
        ]
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
            backend["link_command"] = self.full_command(
                [
                    backend["compiler"]["path"],
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    backend["objects"][0]["path"],
                    "-o",
                    binary["path"],
                    *link_flags,
                ]
            )
        else:
            formal["clang_command"] = self.full_command(
                [
                    "/usr/bin/clang",
                    *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                    *benchmark.BASE_C_FLAGS,
                    formal["generated_c"]["path"],
                    "-o",
                    binary["path"],
                    *link_flags,
                ]
            )
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
        timeout = 0.02 if failure_kind == "timeout" else 5.0
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
                "path": f"/tmp/{workload_id}-reference-{lane}",
                "sha256": ("1" if lane != "go" else "2") * 64,
            }
        return {
            "path": f"/tmp/{workload_id}/{build_mode}/{lane}/project/build/bin/program",
            "sha256": ("3" if lane == "candidate" else "4") * 64,
        }

    def reference_build(self, workload_id: str = "spectral-norm") -> dict:
        workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        link_flags = ["-lm"] if workload.get("link_math") else []
        compiled = {
            lane: {
                "path": f"/tmp/{workload_id}-{lane}{'.cpp' if lane == 'cpp' else '.go' if lane == 'go' else '.c'}",
                "sha256": workload["sources"][lane]["sha256"],
            }
            for lane in benchmark.REFERENCE_LANES
        }
        binaries = {
            lane: self.binary_record(workload_id, "reference", lane)
            for lane in benchmark.REFERENCE_LANES
        }
        c = [
            "/usr/bin/clang",
            *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
            *benchmark.BASE_C_FLAGS,
            compiled["c"]["path"],
            "-o",
            binaries["c"]["path"],
            *link_flags,
        ]
        cpp = [
            "/usr/bin/clang++",
            *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
            *benchmark.BASE_CPP_FLAGS,
            compiled["cpp"]["path"],
            "-o",
            binaries["cpp"]["path"],
            *link_flags,
        ]
        semantic_c = [
            "/usr/bin/clang",
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
                        "/usr/bin/go",
                        "build",
                        "-o",
                        binaries["go"]["path"],
                        compiled["go"]["path"],
                    ]
                ),
            }
        }

    def formal_build(
        self, lane: str, build_mode: str, workload_id: str = "spectral-norm"
    ) -> dict:
        commit = ("a" if lane == "candidate" else "b") * 40
        nomo_sha = ("c" if lane == "candidate" else "d") * 64
        nomo_path = f"/tmp/{lane}/target/release/nomo"
        project = f"/tmp/{workload_id}/{build_mode}/{lane}/project"
        generated_c = f"{project}/build/c/main.c"
        binary = self.binary_record(workload_id, build_mode, lane)
        base = {
            "repository": {"commit": commit},
            "nomo": {"path": nomo_path, "sha256": nomo_sha},
            "source": {"path": "/tmp/main.nomo", "sha256": "f" * 64},
            "lane": lane,
            "binary": binary,
            "compile_time_excluded_from_run_time": True,
        }
        if build_mode == "release":
            return {
                **base,
                "kind": "real-nomo-release",
                "command": self.full_command(
                    [nomo_path, "build", project, "--release"]
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
                    workload_id, generated_c, binary
                ),
                "emit_c_fallback_used": False,
            }
        return {
            **base,
            "kind": "nomo-emit-c-clang",
            "emit_command": self.full_command(
                [nomo_path, "build", project, "--emit-c"]
            ),
            "emit_stdout": "",
            "emit_stderr": "",
            "clang_command": self.full_command(
                [
                    "/usr/bin/clang",
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
                ]
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

    def release_backend(
        self, workload_id: str, generated_c: str, binary: dict
    ) -> dict:
        workload = next(
            item for item in self.manifest["workloads"] if item["id"] == workload_id
        )
        object_path = f"{Path(generated_c).parent}/main.o"
        compiler = {
            "path": "/usr/bin/clang",
            "realpath": "/usr/bin/clang",
            "sha256": "9" * 64,
            "version_output": "Apple clang version 21.0.0",
            "target_triple": "arm64-apple-darwin",
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
                    ]
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
                ]
            ),
            "generated_c": {"path": generated_c, "sha256": "2" * 64},
            "binary": binary,
        }

    def schema_reference_build(self, workload_id: str = "spectral-norm") -> dict:
        reference = self.reference_build(workload_id)
        return {
            "kind": "reference-and-correctness-baseline",
            "source_files": reference["source_files"],
            "compiled_sources": reference["compiled_sources"],
            "commands": {
                name: {
                    **record,
                    "cwd": None,
                    "duration_ns": 1,
                    "exit_code": 0,
                }
                for name, record in reference["commands"].items()
            },
            "compiler_output": {},
            "generated_c": None,
            "binaries": reference["binaries"],
            "compile_time_excluded_from_run_time": True,
        }

    def available_release_lane(self, lane: str) -> dict:
        commit = ("a" if lane == "candidate" else "b") * 40
        binary_sha = ("c" if lane == "candidate" else "d") * 64
        nomo_path = f"/tmp/{lane}/target/release/nomo"
        if self.__class__._rust_toolchain_fixture is None:
            cargo = benchmark.resolve_executable("cargo", "Cargo fixture")
            rustc = benchmark.rustc_for_cargo(cargo)
            self.__class__._rust_toolchain_fixture = {
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
        cargo = rust_toolchain["cargo"]
        rustc = rust_toolchain["rustc"]
        rustc_version = rust_toolchain["rustc_version"]
        rustc_sysroot = rust_toolchain["rustc_sysroot"]
        cargo_version = rust_toolchain["cargo_version"]
        cargo_home = str(
            (Path(f"/tmp/{lane}") / f"{lane}-cargo-home").resolve()
        )
        cargo_environment = {
            "CARGO_TARGET_DIR": str(
                Path(f"/tmp/{lane}/target").resolve()
            ),
            "CARGO_HOME": cargo_home,
            "RUSTC": str(rustc),
        }
        checkout = str(Path(f"/tmp/{lane}").resolve())
        capability = {
            "label": lane,
            "status": "available",
            "reason": "fixture",
            "help_command": self.full_command([nomo_path, "build", "--help"]),
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
            "nomo": {"path": "/tmp/nomo"},
            "clang": {
                "path": "/usr/bin/clang",
                "realpath": "/usr/bin/clang",
                "sha256": "9" * 64,
                "version": "21.0.0",
                "version_output": "Apple clang version 21.0.0",
                "installation": "/usr/bin",
                "target_triple": "arm64-apple-darwin",
                "driver_config_flags": list(
                    benchmark.CLANG_DRIVER_CONFIG_FLAGS
                ),
                "target_command": self.full_command(
                    [
                        "/usr/bin/clang",
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        "-print-target-triple",
                    ]
                ),
            },
            "clangxx": {
                "path": "/usr/bin/clang++",
                "realpath": "/usr/bin/clang++",
                "sha256": "8" * 64,
                "version": "21.0.0",
                "version_output": "Apple clang version 21.0.0",
                "installation": "/usr/bin",
                "target_triple": "arm64-apple-darwin",
                "driver_config_flags": list(
                    benchmark.CLANG_DRIVER_CONFIG_FLAGS
                ),
                "target_command": self.full_command(
                    [
                        "/usr/bin/clang++",
                        *benchmark.CLANG_DRIVER_CONFIG_FLAGS,
                        "-print-target-triple",
                    ]
                ),
            },
            "go": {"path": "/usr/bin/go"},
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
                    "references": self.schema_reference_build(workload),
                    "modes": {},
                }
                for workload in benchmark.WORKLOAD_IDS
            },
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
        for workload in benchmark.WORKLOAD_IDS:
            result["builds"][workload]["references"]["binaries"][
                "nomo-baseline"
            ] = self.binary_record(workload, "reference", "nomo-baseline")
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
        qualification_path.write_text(json.dumps(document), encoding="utf-8")
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
        result = self.completed_result()
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

        def write_file(path: Path, content: bytes) -> dict:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            return {"path": str(path.resolve()), "sha256": v1.sha256_file(path)}

        for lane in ("candidate", "main"):
            state = result["release_lanes"][lane]
            target_dir = root / "compiler-build" / lane
            cargo_home = (
                root / "compiler-build" / f"{lane}-cargo-home"
            )
            compiler = write_file(
                target_dir / "release" / "nomo",
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
            for build_mode in benchmark.FORMAL_BUILD_MODES:
                capability = state["capabilities"][build_mode]
                capability["nomo_path"] = compiler["path"]
                capability["nomo_sha256"] = compiler["sha256"]
                capability["help_command"] = self.full_command(
                    [compiler["path"], "build", "--help"]
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
                    root / "reference-bin" / workload_id / lane,
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
                    ]
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
                    ]
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
                    ]
                ),
                "go_build": self.full_command(
                    [
                        toolchains["go"]["path"],
                        "build",
                        "-o",
                        reference_binaries["go"]["path"],
                        copied_sources["go"]["path"],
                    ]
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
                    generated = write_file(
                        project / "build" / "c" / "main.c",
                        f"{workload_id}-{build_mode}-{lane}-c".encode(),
                    )
                    binary = write_file(
                        project / "build" / "bin" / "program",
                        f"{workload_id}-{build_mode}-{lane}-binary".encode(),
                    )
                    formal["repository"] = state["repository"]
                    formal["nomo"] = {
                        "path": state["nomo_path"],
                        "sha256": state["nomo_sha256"],
                    }
                    nomo_source = (
                        self.suite_root / workload["sources"]["nomo"]["path"]
                    )
                    formal["source"] = {
                        "path": str(nomo_source.resolve()),
                        "sha256": v1.sha256_file(nomo_source),
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
                                    ]
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
                                ]
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
                            ]
                        )
        return benchmark.write_prepared_bundle(
            result, root, self.manifest
        ), root


if __name__ == "__main__":
    unittest.main()
