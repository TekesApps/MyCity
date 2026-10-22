"""Pure controller contract tests; never starts readiness, observer, or model."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CONTROL = REPO / "docs/evidence/m0-t13a-joint-smoke-control-03"
sys.path.insert(0, str(CONTROL))
spec = importlib.util.spec_from_file_location("remaining_controller_run", HERE / "run.py")
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)
from preflight import (SUN_PATH_BYTES, validate_sun_path, verify_freeze,
                       verify_socket_source_inventory)


class RemainingControllerTests(unittest.TestCase):
    def test_formal_plan_is_complete_and_not_claimed_as_run(self):
        plan = json.loads((HERE / "plan.json").read_text())
        self.assertEqual(len(plan["formal_schedule"]), 9)
        self.assertEqual({row["scenario"] for row in plan["formal_schedule"]}, {"l0", "l1", "l2"})
        self.assertEqual({row["run_id"] for row in plan["formal_schedule"]},
                         {"formal01", "formal02", "formal03"})
        self.assertTrue(all(row["state"] == "planned_not_run" for row in plan["formal_schedule"]))
        self.assertEqual(plan["status"], "planned_not_run")

    def test_formal_arguments_and_paths(self):
        for scenario in ("l0", "l1", "l2"):
            for n in (1, 2, 3):
                run_id = f"formal{n:02d}"
                self.assertEqual(run.run_name(scenario, "formal", run_id),
                                 f"st-{scenario}-f{n:02d}")
        self.assertEqual(len({run.run_name(s, "formal", f"formal{n:02d}")
                              for s in run.SCENARIOS for n in (1, 2, 3)}), 9)

    def test_rejects_argument_counterexamples(self):
        for scenario, phase, run_id in (("l3", "formal", "formal01"),
                                        ("l0", "warmup", "formal01"),
                                        ("l0", "formal", "formal04"),
                                        ("l0", "formal", "smoke01"),
                                        ("l0", "smoke", "formal01"),
                                        ("l0", "smoke", "smoke"),
                                        ("l0", "smoke", "smoke100"),
                                        ("l0", "smoke", "smoke01/.."),
                                        ("l0", "smoke", "smoke０１")):
            with self.subTest(scenario=scenario, phase=phase, run_id=run_id):
                with self.assertRaises(ValueError):
                    run.run_name(scenario, phase, run_id)

    def test_socket_path_limit_includes_nul(self):
        safe_len = SUN_PATH_BYTES - 2
        safe = "/" + "x" * (safe_len - 1)
        self.assertTrue(validate_sun_path(safe)["valid"])
        too_long = "/" + "x" * (SUN_PATH_BYTES - 1)
        with self.assertRaises(AssertionError):
            validate_sun_path(too_long)

    def test_source_and_support_identity_are_fixed(self):
        self.assertEqual(run.SOURCE["fixed_source_sha256"],
                         "32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2")
        self.assertEqual(run.SOURCE["fixed_binary_sha256"],
                         "59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0")
        self.assertEqual(set(run.unique_support_hashes()), set(run.SUPPORT))
        self.assertEqual(len(run.SUPPORT), 16)
        self.assertTrue({"source.json", "freeze.json", "unix-socket-paths.json"}.issubset(run.SUPPORT))
        self.assertTrue(all(len(value) == 64 for value in run.unique_support_hashes().values()))
        with self.assertRaises(RuntimeError):
            run.validate_support_hashes({"observe.py": "changed"}, {"observe.py": "published"})
        for changed in ("source.json", "freeze.json"):
            expected = {name: f"published-{name}" for name in run.SUPPORT}
            missing = dict(expected)
            missing.pop(changed)
            with self.subTest(missing=changed), self.assertRaises(RuntimeError):
                run.validate_support_hashes(missing, expected)
            altered = dict(expected)
            altered[changed] = "tampered"
            with self.subTest(altered=changed), self.assertRaises(RuntimeError):
                run.validate_support_hashes(altered, expected)
        self.assertEqual(len(run.SUPPORT_MANIFEST_SHA256), 64)

    def test_readiness_uses_only_final_clean_window(self):
        early_contamination = run.readiness_gate(0, 240, [10, 50, 119])
        self.assertEqual(early_contamination["status"], "passed")
        recent_contamination = run.readiness_gate(0, 240, [10, 121])
        self.assertEqual(recent_contamination["reason"], "compiler_observed_in_final_120_seconds")
        readiness_failure = run.readiness_gate(2, 240, [])
        self.assertEqual(readiness_failure["reason"], "readiness_exit_nonzero")

    def test_frozen_steady_command_and_durations(self):
        contract = run.static_contract(run.ARCHIVE)
        self.assertEqual(contract["duration_contract_seconds"],
                         {"formal_warmup": 300, "formal_measured": 1800,
                          "formal_quiesce": 10})
        self.assertTrue(contract["supervisor_owns_normal_pool"])
        self.assertTrue(contract["l1_is_external_charge_only"])
        self.assertTrue(contract["l2_uses_internal_broker_link"])

    def test_frozen_source_and_all_formal_socket_paths(self):
        frozen = verify_freeze(run.SOURCE, run.FREEZE, run.ARCHIVE,
                               run.SOURCE["fixed_binary"], run.REPO)
        self.assertEqual(frozen["source_sha256"], run.SOURCE["fixed_source_sha256"])
        names = [run.run_name(scenario, "formal", f"formal{n:02d}")
                 for scenario in ("l0", "l1", "l2") for n in (1, 2, 3)]
        sockets = verify_socket_source_inventory(run.ARCHIVE, run.WORK_BASE, tuple(names))
        self.assertTrue(sockets["all_bind_and_connect_paths_fit"])
        self.assertEqual(len(sockets["run_paths"]), 18)
        self.assertTrue(all(row["normal_pool_socket"]["utf8_bytes_plus_nul"] < SUN_PATH_BYTES
                            for row in sockets["run_paths"]))

    def test_environment_cleaning_is_scoped_to_mycity_names(self):
        env = {"MYCITY_HEAP_LOG": "inherited", "MYCITY_OBSERVER_CLOCK": "stale",
               "PATH": "/usr/bin", "OTHER": "keep"}
        removed = run.clean_mycity_environment(env)
        self.assertEqual(removed, ["MYCITY_HEAP_LOG", "MYCITY_OBSERVER_CLOCK"])
        self.assertEqual(env, {"PATH": "/usr/bin", "OTHER": "keep"})

    def test_wrapper_execs_once_without_restart_or_pass_claim(self):
        source = (HERE / "run.py").read_text()
        self.assertIn("os.execv(sys.executable, observer)", source)
        self.assertIn("never automatically restart", source)
        self.assertNotIn("while True", source)
        self.assertNotIn('"status": "passed"', source)


if __name__ == "__main__":
    unittest.main()
