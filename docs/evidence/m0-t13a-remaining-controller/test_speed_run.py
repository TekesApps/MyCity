"""Pure speed-controller checks; does not invoke readiness or a fixed binary."""
import importlib.util
import json
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
CONTROL = HERE.parent / "m0-t13a-joint-smoke-control-03"
sys.path.insert(0, str(CONTROL))
spec = importlib.util.spec_from_file_location("remaining_speed_controller", HERE / "speed_run.py")
speed_run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(speed_run)


class SpeedControllerTests(unittest.TestCase):
    def test_five_profiles_have_unique_planned_directories(self):
        expected = {"600": "sp-600-01", "60": "sp-60-01", "10": "sp-10-01",
                    "1": "sp-1-01", "unpaced": "sp-u-01"}
        paths = {rate: speed_run.speed_name(rate, "speed01") for rate in speed_run.SPEEDS}
        self.assertEqual(paths, expected)
        self.assertEqual(len(set(paths.values())), 5)
        plan = speed_run.speed_plan()
        self.assertEqual(len(plan), 5)
        self.assertTrue(all(row["state"] == "planned_not_run" for row in plan))

    def test_rejects_bad_rates_run_ids_and_long_or_path_inputs(self):
        for rate, run_id in (("step", "speed01"), ("60x", "speed01"),
                             ("60", "speed00"), ("60", "speed100"),
                             ("60", "speed01/.."), ("60", "speed０１"),
                             ("60", "s" * 10000)):
            with self.subTest(rate=rate[:20], run_id=run_id[:24]):
                with self.assertRaises(ValueError):
                    speed_run.speed_name(rate, run_id)
        from preflight import validate_sun_path
        too_long = "/" + "x" * 104
        with self.assertRaises(AssertionError):
            validate_sun_path(too_long)

    def test_fixed_source_identity_rejects_mismatch(self):
        source = speed_run.steady.SOURCE
        result = speed_run.validate_source_identity(
            source, source["fixed_source_sha256"], source["fixed_binary_sha256"])
        self.assertTrue(result["matches_fixed_identity"])
        changed = dict(source, fixed_source_sha256="0" * 64)
        with self.assertRaises(RuntimeError):
            speed_run.validate_source_identity(changed, source["fixed_source_sha256"],
                                               source["fixed_binary_sha256"])
        changed_binary = dict(source, fixed_binary_sha256="0" * 64)
        with self.assertRaises(RuntimeError):
            speed_run.validate_source_identity(changed_binary, source["fixed_source_sha256"],
                                               source["fixed_binary_sha256"])

    def test_fixed_speed_contract_and_clock_gap_are_explicit(self):
        contract = speed_run.inspect_speed_contract(speed_run.steady.ARCHIVE)
        self.assertEqual(contract["entrypoint"],
                         "speed-run NEW_MODEL_DIR 600|60|10|1|unpaced")
        self.assertEqual(contract["selected_entrypoint"], "speed-run")
        self.assertNotIn("pe0-speed-driver", contract["selected_entrypoint"])
        self.assertFalse(contract["observer_clock_handshake"]["compatible"])
        self.assertTrue(contract["observer_clock_handshake"]["observer_requires_nonce_bound_startup_exchange"])
        self.assertFalse(contract["model_launch_permitted"])
        self.assertIn("observer_clock::synchronize", contract["observer_clock_handshake"]["reason"])
        self.assertFalse(any(row["source"].endswith("scale_speed.rs")
                             for row in contract["observer_clock_handshake"]["synchronize_call_sites_in_fixed_source"]))
        self.assertEqual(contract["workload"]["full_days"], 30)
        self.assertEqual(contract["workload"]["hours"], 720)
        self.assertEqual(contract["workload"]["targets"], 723)
        self.assertEqual(contract["workload"]["worker_windows"], 724)
        self.assertEqual(contract["workload"]["l1"], "absent")
        self.assertFalse(contract["workload"]["completion_is_speed_acceptance"])
        self.assertEqual(len(contract["observer_clock_handshake"]["synchronize_call_sites_in_fixed_source"]), 5)

    def test_all_five_speed_paths_fit_published_unix_socket_limit(self):
        from preflight import SUN_PATH_BYTES, verify_socket_source_inventory
        names = [speed_run.speed_name(rate, "speed01") for rate in speed_run.SPEEDS]
        result = verify_socket_source_inventory(speed_run.steady.ARCHIVE,
                                                speed_run.steady.WORK_BASE, tuple(names))
        self.assertEqual(len(result["run_paths"]), 10)
        self.assertTrue(all(row["normal_pool_socket"]["utf8_bytes_plus_nul"] < SUN_PATH_BYTES
                            for row in result["run_paths"]))

    def test_blocked_wrapper_keeps_existing_integrity_gates(self):
        source = (HERE / "speed_run.py").read_text()
        for token in ("verify_freeze(", "verify_storage(", "verify_socket_source_inventory(",
                      "unique_support_hashes()", "monitored_readiness(",
                      "clean_mycity_environment(os.environ)", "measurement-requested",
                      "os.execv(sys.executable"):
            self.assertIn(token, source)
        self.assertIn("if not contract[\"model_launch_permitted\"]", source)
        self.assertIn("blocked before readiness/model launch", source)


if __name__ == "__main__":
    unittest.main()
