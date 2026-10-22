import importlib.util
import unittest
from pathlib import Path

RUNNER = Path(__file__).with_name("run_speed.py")
spec = importlib.util.spec_from_file_location("speed_full_v125_runner", RUNNER)
speed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(speed)


class SpeedRunnerPureTests(unittest.TestCase):
    def test_only_two_full_rates_and_unique_ids_are_admitted(self):
        self.assertEqual(speed.attempt_name("1", "speed01"), "rate1-01")
        self.assertEqual(speed.attempt_name("10", "speed01"), "rate10-01")
        for args in (("600", "speed01"), ("unpaced", "speed01"), ("1", "speed02")):
            with self.assertRaises(ValueError):
                speed.attempt_name(*args)

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

    def test_ps_pair_preserves_paths_and_arguments_with_spaces(self):
        comm = (
            " 501 1 Fri Oct 23 04:09:42 2026 /Volumes/HP P900/tool chains/mycity-headless\n"
            " 502 1 Fri Oct 23 04:09:43 2026 /Users/frankqin/rust tools/bin/cargo\n"
        )
        args = (
            " 501 1 Fri Oct 23 04:09:42 2026 /Volumes/HP P900/tool chains/mycity-headless"
            " speed-run /Volumes/HP P900/work root/model full 1\n"
            " 502 1 Fri Oct 23 04:09:43 2026 /Users/frankqin/rust tools/bin/cargo"
            " build --manifest-path /Users/frankqin/My City/Cargo.toml\n"
        )
        rows = speed.parse_ps_pair(comm, args)
        self.assertEqual(rows[0]["comm"], "/Volumes/HP P900/tool chains/mycity-headless")
        self.assertEqual(rows[0]["args"], "/Volumes/HP P900/tool chains/mycity-headless"
                         " speed-run /Volumes/HP P900/work root/model full 1")
        self.assertEqual(rows[1]["comm"], "/Users/frankqin/rust tools/bin/cargo")
        activity = speed.process_activity(rows)
        self.assertEqual([row["pid"] for row in activity["heavy_mycity"]], [501])
        self.assertEqual([row["pid"] for row in activity["compilers"]], [502])

    def test_ps_pair_fails_closed_when_pid_or_start_identity_changes(self):
        comm = " 501 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless\n"
        changed_pid = " 502 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless run\n"
        changed_start = " 501 1 Fri Oct 23 04:09:43 2026 /tmp/mycity-headless run\n"
        for changed in (changed_pid, changed_start):
            with self.subTest(changed=changed), self.assertRaisesRegex(
                    RuntimeError, "PID/start identities changed"):
                speed.parse_ps_pair(comm, changed)

    def test_ps_pair_fails_closed_when_parent_changes(self):
        comm = " 501 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless\n"
        args = " 501 9 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless run\n"
        with self.assertRaisesRegex(RuntimeError, "PPID changed"):
            speed.parse_ps_pair(comm, args)

    def test_ps_pair_ignores_only_its_transient_bulk_ps_processes(self):
        comm = (
            " 501 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless\n"
            " 601 1 Fri Oct 23 04:09:43 2026 /bin/ps\n"
        )
        args = (
            " 501 1 Fri Oct 23 04:09:42 2026 /tmp/mycity-headless run\n"
            " 602 1 Fri Oct 23 04:09:44 2026 /bin/ps -axo pid=,ppid=,lstart=,args=\n"
        )
        rows = speed.parse_ps_pair(comm, args)
        self.assertEqual([row["pid"] for row in rows], [501])

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
        for name in ("rate1-01", "rate10-01"):
            paths = speed.socket_preflight(name)
            self.assertLess(paths["normal_pool_socket"]["utf8_bytes_plus_nul"], 104)
            self.assertLess(paths["max_formatted_broker_socket"]["utf8_bytes_plus_nul"], 104)


if __name__ == "__main__":
    unittest.main()
