import io
import hashlib
import json
import unittest
import zlib

from frame_codec import BLOCK_FRAMES, FrameWriter, HEADER, MAGIC, LogCapExceeded, MAX_BLOCK_RAW_BYTES, MAX_FRAME_RAW_BYTES, MAX_PACKED_BYTES, encode_block, iter_blocks
from log_budget import LOG_CAP_BYTES, project
from preflight import stream_source_sha256, unix_path_bytes, validate_sun_path
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
        rows = {201: (1, None, "1", "/Users/frankqin/"), 202: (1, None, "1", "mycity-headless")}
        looked_up = []

        def path_lookup(pid):
            looked_up.append(pid)
            return "/Users/frankqin/.rustup/toolchains/1.95.0/bin/cargo" if pid == 201 else None

        found = observer_compilers(rows, path_lookup)
        self.assertEqual([row["pid"] for row in found], [201])
        self.assertEqual(looked_up, [201])

    def test_lossless_four_frame_blocks_preserve_original_jsonl_bytes(self):
        frames = [json.dumps({"i": i, "complete": i != 2}, separators=(",", ":")).encode() + b"\n" for i in range(9)]
        sink = io.BytesIO()
        writer = FrameWriter(sink, 256 * 1024**2, lambda: 0)
        for frame in frames:
            writer.add(frame)
        writer.flush()
        decoded = [frame for block in iter_blocks(io.BytesIO(sink.getvalue())) for frame in block["frames"]]
        self.assertEqual(decoded, frames)
        self.assertEqual(writer.block_count, 3)
        self.assertGreater(writer.max_encoded_block_bytes, 0)

    def test_codec_rejects_crc_corruption_and_truncated_tail(self):
        encoded = bytearray(MAGIC + encode_block([b'{"a":1}\n']))
        encoded[-1] ^= 1
        with self.assertRaises((ValueError, zlib.error)):
            list(iter_blocks(io.BytesIO(encoded)))
        with self.assertRaises(ValueError):
            list(iter_blocks(io.BytesIO(MAGIC + b"\x01")))

    def test_near_cap_candidate_is_rejected_before_stream_growth(self):
        frame = b'{"near":"cap"}\n'
        encoded_size = len(encode_block([frame]))
        sink = io.BytesIO()
        writer = FrameWriter(sink, len(MAGIC) + 100 + encoded_size - 1, lambda: 100)
        before = sink.getvalue()
        with self.assertRaises(LogCapExceeded):
            writer.add(frame)
        self.assertEqual(sink.getvalue(), before)
        self.assertEqual(writer.pending, [])

    def test_incompressible_full_block_is_admitted_before_disk_growth(self):
        payload = b"".join(hashlib.sha256(i.to_bytes(8, "little")).digest() for i in range(1024))
        frames = [payload[i * 8192:(i + 1) * 8192] + b"\n" for i in range(4)]
        candidate_size = len(encode_block(frames))
        self.assertGreater(candidate_size, sum(map(len, frames)))
        sink = io.BytesIO()
        writer = FrameWriter(sink, len(MAGIC) + candidate_size - 1, lambda: 0)
        for frame in frames[:3]:
            writer.add(frame)
        before = sink.getvalue()
        with self.assertRaises(LogCapExceeded):
            writer.add(frames[3])
        self.assertEqual(sink.getvalue(), before)
        self.assertEqual(len(writer.pending), 3)

    def test_partial_tail_flush_rechecks_latest_latency_without_writing(self):
        state = {"latency": 0}
        frames = [b'{"tail":1}\n', b'{"tail":2}\n']
        candidate_size = len(encode_block(frames))
        sink = io.BytesIO()
        writer = FrameWriter(sink, len(MAGIC) + candidate_size + 10, lambda: state["latency"])
        for frame in frames:
            writer.add(frame)
        state["latency"] = 11
        before = sink.getvalue()
        with self.assertRaises(LogCapExceeded):
            writer.flush()
        self.assertEqual(sink.getvalue(), before)
        self.assertEqual(len(writer.pending), 2)

    def test_decoder_rejects_advertised_raw_and_packed_sizes_before_reading_payload(self):
        oversized_raw = HEADER.pack(1, MAX_BLOCK_RAW_BYTES + 1, 1, 0)
        with self.assertRaises(ValueError):
            list(iter_blocks(io.BytesIO(MAGIC + oversized_raw)))
        oversized_packed = HEADER.pack(1, 1, MAX_PACKED_BYTES + 1, 0)
        with self.assertRaises(ValueError):
            list(iter_blocks(io.BytesIO(MAGIC + oversized_packed)))

    def test_encoder_rejects_frame_over_explicit_sampling_frame_bound(self):
        with self.assertRaises(ValueError):
            encode_block([b"x" * (MAX_FRAME_RAW_BYTES + 1) + b"\n"])

    def test_decoder_limits_expansion_to_advertised_raw_size(self):
        inflater = zlib.compressobj(wbits=-zlib.MAX_WBITS)
        compressed = inflater.compress(b"A" * 1_000_000) + inflater.flush()
        payload = HEADER.pack(1, 1, len(compressed), 0) + compressed
        with self.assertRaises(ValueError):
            list(iter_blocks(io.BytesIO(MAGIC + payload)))

    def test_capacity_projection_scales_fixed_business_and_scan_based_layout_separately(self):
        result = project(2_441, 2_684_183, [1_932] * 611, 49_792_887, 1_161_000, 600)
        self.assertEqual(result["formal_expected_frame_count"], 115_200)
        self.assertEqual(result["formal_expected_block_count"], 28_800)
        self.assertEqual(result["formal_business_compute_durations_estimate_bytes"], 49_792_887)
        self.assertEqual(result["formal_layout_compute_durations_estimate_bytes"], 55_728_000)
        self.assertEqual(result["formal_combined_log_upper_estimate_bytes"], 161_162_495)
        self.assertEqual(result["cap_estimate"], "conservative_projection_under_hard_cap")
        self.assertEqual(result["hard_cap_bytes"], LOG_CAP_BYTES)

    def test_peak_block_projection_catches_spike_hidden_by_average(self):
        result = project(8, 1_000, [100, 100, 100, 100, 100, 100, 100, 20_000], 0, 0, 1, target_seconds=8, target_scans=1)
        average_based = round(sum([100] * 7 + [20_000]) / 8 * 8)
        self.assertGreater(result["formal_host_log_upper_estimate_bytes"], average_based)

    def test_projection_rejects_exceeding_original_cap_without_relaxation(self):
        result = project(4, 100, [LOG_CAP_BYTES // 2], 0, 0, 600, target_seconds=4, target_scans=600)
        self.assertEqual(result["cap_estimate"], "conservative_projection_exceeds_hard_cap")
        self.assertEqual(result["hard_cap_bytes"], 256 * 1024**2)

    def test_projection_accepts_incomplete_frame_count_as_retained_capacity_data(self):
        # Completeness is a physical-identity report, not a new functional gate;
        # every raw frame length, including incomplete identities, is projected.
        frames = [100, 200, 300, 400]
        result = project(len(frames), sum(frames), [900], 0, 0, 600, target_seconds=1, target_scans=600)
        self.assertEqual(result["smoke_frame_count"], len(frames))
        self.assertEqual(result["formal_host_log_upper_estimate_bytes"], 908)

    def test_macos_socket_path_limit_counts_utf8_bytes_and_terminating_nul(self):
        self.assertEqual(unix_path_bytes("a" * 102), 103)
        self.assertTrue(validate_sun_path("a" * 102)["valid"])
        self.assertEqual(unix_path_bytes("中" * 34), 103)
        self.assertTrue(validate_sun_path("中" * 34)["valid"])
        with self.assertRaises(AssertionError):
            validate_sun_path("a" * 103)
        with self.assertRaises(AssertionError):
            validate_sun_path("中" * 35)


if __name__ == "__main__":
    unittest.main()
