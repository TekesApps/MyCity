import unittest

from log_budget import forecast
from preflight import stream_source_sha256
from process_activity import is_compiler, observer_compilers, parse_process_listing


class HarnessPureValidationTests(unittest.TestCase):
    def test_source_hash_uses_sorted_names_and_little_endian_length_prefixes(self):
        left = [("z", b"last"), ("a", b"first")]
        right = list(reversed(left))
        self.assertEqual(stream_source_sha256(left), stream_source_sha256(right))
        self.assertNotEqual(stream_source_sha256(left), stream_source_sha256([("a", b"firstx"), ("z", b"last")]))

    def test_external_compiler_detection_is_exact_and_excludes_requested_pid(self):
        listing = """\
101 1 /opt/rust/bin/cargo /opt/rust/bin/cargo +1.95.0 check --workspace
102 1 rustc rustc --crate-name city
103 1 mycity-headless /tmp/mycity-headless joint-endurance
104 1 rg rg cargo
"""
        rows = parse_process_listing(listing, exclude_pids=(101,))
        self.assertEqual([row["pid"] for row in rows], [102])
        self.assertTrue(is_compiler("/opt/rust/bin/cargo"))
        self.assertFalse(is_compiler("mycity-headless", "mycity-headless cargo"))

    def test_observer_resolves_truncated_process_names_without_sampler_children(self):
        rows = {
            201: (1, None, "1", "/Users/frankqin/"),
            202: (1, None, "1", "mycity-headless"),
        }
        looked_up = []

        def path_lookup(pid):
            looked_up.append(pid)
            return "/Users/frankqin/.rustup/toolchains/1.95.0/bin/cargo" if pid == 201 else None

        found = observer_compilers(rows, path_lookup)
        self.assertEqual([row["pid"] for row in found], [201])
        self.assertEqual(looked_up, [201])

    def test_formal_projection_keeps_four_hz_and_never_expands_fixed_cap(self):
        result = forecast([100, 140, 120], latency_bytes=1_000, target_seconds=600)
        self.assertEqual(result["sample_hz_unchanged"], 4)
        self.assertEqual(result["formal_expected_frame_count"], 2_400)
        self.assertEqual(result["formal_host_log_upper_estimate_bytes"], 336_000)
        self.assertEqual(result["formal_compute_durations_estimate_bytes"], 1_000)
        self.assertEqual(result["hard_cap_bytes"], 256 * 1024**2)
        self.assertEqual(result["cap_estimate"], "conservative_projection_under_hard_cap")

    def test_projection_refuses_missing_or_invalid_frames(self):
        for frames in ([], [0], [-1]):
            with self.subTest(frames=frames), self.assertRaises(ValueError):
                forecast(frames, latency_bytes=0)


if __name__ == "__main__":
    unittest.main()
