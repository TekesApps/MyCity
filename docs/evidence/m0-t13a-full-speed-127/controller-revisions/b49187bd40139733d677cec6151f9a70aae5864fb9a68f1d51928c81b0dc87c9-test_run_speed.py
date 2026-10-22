import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

RUNNER = Path(__file__).with_name("run_speed.py")
spec = importlib.util.spec_from_file_location("speed_full_v125_runner", RUNNER)
speed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(speed)


class SpeedRunnerPureTests(unittest.TestCase):
    def test_reviewed_rate_run_ids_map_to_unique_attempts(self):
        self.assertEqual(speed.attempt_name("1", "speed01"), "rate1-01")
        self.assertEqual(speed.attempt_name("10", "speed01"), "rate10-01")
        self.assertEqual(speed.attempt_name("10", "speed02"), "rate10-02")
        self.assertNotEqual(speed.attempt_name("10", "speed02"),
                            speed.attempt_name("10", "speed01"))
        for args in (("600", "speed01"), ("unpaced", "speed01"), ("1", "speed02"),
                     ("10", "speed03")):
            with self.assertRaises(ValueError):
                speed.attempt_name(*args)

    def test_existing_attempt_root_cannot_be_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "rate10-02").mkdir()
            with patch.object(speed, "ATTEMPT_BASE", base):
                with self.assertRaises(FileExistsError):
                    speed.run_one("10", "speed02")
            self.assertEqual(sorted(p.name for p in base.iterdir()), ["rate10-02"])

    def test_fixed_source_or_binary_mismatch_fails_closed(self):
        good = {"source_sha256": speed.FIXED_SOURCE_SHA,
                "binary_sha256": speed.FIXED_BINARY_SHA,
                "source_zip_sha256": speed.FIXED_ARCHIVE_SHA}
        self.assertTrue(speed.validate_fixed_record(good))
        for key in good:
            changed = dict(good)
            changed[key] = "0" * 64
            with self.assertRaises(RuntimeError):
                speed.validate_fixed_record(changed)

    def test_clock_map_source_pid_nonce_and_bounds_are_fail_closed(self):
        child = {"identity": {"pid": 42}}
        clock = {"source_sha256": speed.FIXED_SOURCE_SHA, "supervisor_pid": 42,
                 "nonce": "a" * 32, "request_sent_observer_ns": 10,
                 "reply_received_observer_ns": 20,
                 "supervisor_origin_observer_ns_min": 5,
                 "supervisor_origin_observer_ns_max": 8, "uncertainty_ns": 3}
        self.assertTrue(speed.validate_clock_map(clock, child))
        for key, wrong in (("source_sha256", "0" * 64), ("supervisor_pid", 41),
                           ("nonce", "bad"), ("uncertainty_ns", 4)):
            changed = dict(clock)
            changed[key] = wrong
            self.assertFalse(speed.validate_clock_map(changed, child))

    def test_socket_length_counts_utf8_bytes_and_nul(self):
        self.assertEqual(speed.socket_bytes_plus_nul("a" * 102), 103)
        self.assertEqual(speed.socket_bytes_plus_nul("a" * 103), 104)
        self.assertEqual(speed.socket_bytes_plus_nul("路"), 4)

    def test_process_activity_identifies_only_compilers_and_mycity_heavy_names(self):
        rows = [
            {"pid": 1, "ppid": 0, "start_identity": "t", "comm": "/usr/bin/cargo", "args": "cargo build"},
            {"pid": 2, "ppid": 0, "start_identity": "t", "comm": "/tmp/mycity-headless", "args": "mycity-headless speed-run"},
            {"pid": 3, "ppid": 0, "start_identity": "t", "comm": "/usr/bin/python3", "args": "python3 observer.py"},
        ]
        got = speed.process_activity(rows)
        self.assertEqual([r["pid"] for r in got["compilers"]], [1])
        self.assertEqual([r["pid"] for r in got["heavy_mycity"]], [2])

    def test_ps_rows_uses_one_projection_and_preserves_spaced_comm(self):
        output = (
            " 501 1 Fri Oct 23 04:09:42 2026 /Volumes/HP P900/tool chains/mycity-headless\n"
            " 502 1 Fri Oct 23 04:09:43 2026 /Users/frankqin/rust tools/bin/cargo\n"
        )
        with patch.object(speed.subprocess, "check_output", return_value=output) as check:
            rows = speed.ps_rows()
        check.assert_called_once_with(
            ["/bin/ps", "-axo", "pid=,ppid=,lstart=,comm="], text=True)
        self.assertEqual(rows[0]["comm"], "/Volumes/HP P900/tool chains/mycity-headless")
        self.assertEqual(rows[1]["comm"], "/Users/frankqin/rust tools/bin/cargo")
        activity = speed.process_activity(rows)
        self.assertEqual([row["pid"] for row in activity["heavy_mycity"]], [501])
        self.assertEqual([row["pid"] for row in activity["compilers"]], [502])

    def test_process_birth_or_exit_between_calls_does_not_join_stale_snapshots(self):
        first = " 501 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless\n"
        second = first + " 777 1 Fri Oct 23 04:10:02 2026 /tmp/unrelated-helper\n"
        with patch.object(speed.subprocess, "check_output", side_effect=(first, second)) as check:
            rows_before = speed.ps_rows()
            rows_after = speed.ps_rows()
        self.assertEqual([r["pid"] for r in rows_before], [501])
        self.assertEqual([r["pid"] for r in rows_after], [501, 777])
        self.assertEqual(check.call_count, 2)

    def test_ps_listing_rejects_malformed_and_duplicate_pid_start_rows(self):
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            speed.parse_ps_listing(" 501 1 Fri Oct 23 04:09:42 2026\n")
        duplicate = (
            " 501 1 Fri Oct 23 04:09:42 2026 /tmp/a\n"
            " 501 2 Fri Oct 23 04:09:42 2026 /tmp/b\n"
        )
        with self.assertRaisesRegex(RuntimeError, "duplicate PID/start"):
            speed.parse_ps_listing(duplicate)

    def test_runtime_process_exclusion_is_limited_to_own_observer_tree(self):
        rows = [{"pid": 10, "ppid": 1}, {"pid": 11, "ppid": 10},
                {"pid": 12, "ppid": 11}, {"pid": 20, "ppid": 1}]
        self.assertEqual(speed.descendant_pids(rows, 10), {10, 11, 12})

    def test_controller_declares_deadline_greater_than_rust_child_wait(self):
        self.assertEqual(speed.CHILD_WAIT_SECONDS, 3000)
        self.assertGreater(speed.CONTROLLER_DEADLINE_SECONDS, speed.CHILD_WAIT_SECONDS)

    def test_run_path_does_not_precreate_observed_or_model(self):
        source = RUNNER.read_text()
        self.assertIn("observed = attempt / \"observed\"  # Deliberately not created here", source)
        self.assertNotIn("observed.mkdir", source)
        self.assertNotIn("(observed / \"model\").mkdir", source)

    def test_fixed_full_source_contract_and_actual_external_socket_paths(self):
        contract = speed.inspect_speed_contract(speed.ARCHIVE)
        self.assertTrue(all(contract["checks"].values()))
        self.assertEqual(contract["workload"], {"days": 30, "hours": 720, "targets": 723,
                                                "worker_windows": 724, "tier": "full",
                                                "l1": "absent", "formal_m0": False,
                                                "rate_semantics": "1|10 wall seconds per game day; fixed source's timeout resolves to 3000s for both"})
        for name in ("rate1-01", "rate10-01", "rate10-02"):
            paths = speed.socket_preflight(name)
            self.assertLess(paths["normal_pool_socket"]["utf8_bytes_plus_nul"], 104)
            self.assertLess(paths["max_formatted_broker_socket"]["utf8_bytes_plus_nul"], 104)


if __name__ == "__main__":
    unittest.main()
