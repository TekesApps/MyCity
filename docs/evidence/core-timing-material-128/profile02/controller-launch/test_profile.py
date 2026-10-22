import importlib.util
import json
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

RUNNER = Path(__file__).with_name("run_profile.py")
spec = importlib.util.spec_from_file_location("timing_full_v128_runner", RUNNER)
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


class TimingProfilePureTests(unittest.TestCase):
    def test_source_stream_hash_is_sorted_and_length_prefixed(self):
        rows = [("b", b"xyz"), ("a", b"x")]
        self.assertEqual(profile.source_stream_sha256(rows), profile.source_stream_sha256(list(reversed(rows))))
        self.assertNotEqual(profile.source_stream_sha256([("ab", b"c")]),
                            profile.source_stream_sha256([("a", b"bc")]))

    def test_clock_map_requires_source_child_pid_nonce_and_consistent_bounds(self):
        child = {"identity": {"pid": 84}}
        good = {"source_sha256": "a" * 64, "supervisor_pid": 84, "nonce": "b" * 32,
                "request_sent_observer_ns": 1, "reply_received_observer_ns": 3,
                "supervisor_origin_observer_ns_min": 4,
                "supervisor_origin_observer_ns_max": 9, "uncertainty_ns": 5}
        self.assertTrue(profile.validate_clock(good, child, "a" * 64))
        for key, value in (("source_sha256", "c" * 64), ("supervisor_pid", 85),
                           ("nonce", "bad"), ("uncertainty_ns", 4)):
            changed = dict(good, **{key: value})
            self.assertFalse(profile.validate_clock(changed, child, "a" * 64))

    def test_process_identity_sampling_exception_is_preserved_as_unknown(self):
        class BrokenIdentity:
            @staticmethod
            def ps_identity(_pid):
                raise OSError("synthetic ps failure")

        identity, error = profile.safe_ps_identity(BrokenIdentity, 84)
        self.assertIsNone(identity)
        self.assertEqual(error, "synthetic ps failure")

    def test_empty_observer_identity_is_reported_as_unknown(self):
        self.assertIsNone(profile.observer_child_pid({}))
        self.assertIsNone(profile.observer_child_pid({"identity": None}))
        self.assertEqual(profile.observer_child_pid({"identity": {"pid": 84}}), 84)

    def test_readiness_exception_terminates_and_waits_only_its_popen_child(self):
        class Child:
            pid = 999

            def __init__(self):
                self.terminated = False
                self.waited = False

            def poll(self):
                return None if not self.terminated else 0

            def terminate(self):
                self.terminated = True

            def wait(self, timeout):
                self.waited = True
                return 0

        child = Child()
        fake_subprocess = types.SimpleNamespace(Popen=lambda *a, **k: child)

        def raise_after_start(*_args):
            fake_subprocess.Popen(["readiness"])
            raise RuntimeError("synthetic gate error")

        fake_speed = types.SimpleNamespace(subprocess=fake_subprocess, readiness_gate=raise_after_start)
        readiness = {}
        with self.assertRaisesRegex(RuntimeError, "synthetic gate error"):
            profile.guarded_readiness_gate(fake_speed, Path("unused"), readiness, {})
        self.assertTrue(child.terminated)
        self.assertTrue(child.waited)
        self.assertEqual(readiness["owned_popen_cleanup"], [
            {"pid": 999, "action": "terminate_own_readiness_helper"}])

    def test_real_diagnostic_record_and_fixed_full_workload_are_hash_bound(self):
        result = profile.verify_diagnostic_record()
        record = json.loads(profile.BINARY_RECORD.read_text())
        self.assertEqual(result["source_sha256"], record["source_sha256"])
        self.assertEqual(result["binary_sha256"], record["binary_sha256"])
        self.assertEqual(result["source_zip_sha256"], record["source_zip_sha256"])
        self.assertTrue(all(result["full_unpaced_workload_checks"].values()))
        self.assertEqual(result["acceptance_fixture_sha256"], profile.ACCEPTANCE_SHA)

    def test_external_work_root_and_actual_socket_path_are_ready(self):
        speed = profile.load_helper()
        storage = speed.verify_storage(profile.ROOT)
        sockets = speed.socket_preflight("", work_base=profile.ROOT)
        self.assertEqual(storage["volume"]["volume_uuid"], "4289CF6D-F09D-4831-8558-4AB223E4EE04")
        self.assertLess(sockets["normal_pool_socket"]["utf8_bytes_plus_nul"], profile.SUN_PATH_BYTES)

    def test_script_does_not_precreate_observed_or_model(self):
        source = RUNNER.read_text()
        self.assertNotIn("observed.mkdir", source)
        self.assertNotIn("model.mkdir", source)
        self.assertNotIn("mkdir(parents=True", source)
        self.assertIn('"observed/model"', source)

    def test_run_root_is_one_shot_and_refuses_existing_receipts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "intent.json").write_text(json.dumps({"state": "old"}))
            with patch.object(profile, "ROOT", root):
                with self.assertRaisesRegex(RuntimeError, "already used"):
                    profile.run()

    def test_run_root_is_the_new_unique_attempt(self):
        self.assertEqual(profile.ROOT.name, "timing-full-v128-02")
        self.assertFalse((profile.ROOT / "observed").exists())
        self.assertFalse((profile.ROOT / "model").exists())


if __name__ == "__main__":
    unittest.main()
