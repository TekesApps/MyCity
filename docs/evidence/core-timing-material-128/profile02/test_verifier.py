"""Synthetic unit checks for timing transcript aggregation; no model is run."""
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


VERIFIER_PATH = Path(__file__).with_name("verify_profile.py")
SPEC = importlib.util.spec_from_file_location("verify_profile", VERIFIER_PATH)
assert SPEC is not None and SPEC.loader is not None
verify_profile = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify_profile)


PHASES = verify_profile.TIMING_PHASES


def timing(value: int) -> dict:
    result = {"schema": "mycity-sim-timing-v1", "scope_notes": {"contract": "synthetic"}}
    result.update({phase: {"count": value, "total_ns": value * 10, "max_ns": value * 10}
                   for phase in PHASES})
    return result


def write_transcript(path: Path, *, missing_at: int | None = None,
                     regress_at: int | None = None) -> None:
    with path.open("w", encoding="utf-8") as stream:
        for request_id in range(723):
            stream.write(json.dumps({"data": {
                "request": request_id,
                "command": {"kind": "status"},
            }}) + "\n")
            is_second_process = request_id >= 721
            local_index = request_id - 721 if is_second_process else request_id
            value = local_index + 1
            if regress_at == request_id:
                value = max(0, local_index - 1)
            response = {
                "response": request_id,
                "kind": "result",
                "result_ok": True,
                "sim_clock": {
                    "pid": 200 if is_second_process else 100,
                    "instance_unix_ns": 2_000 if is_second_process else 1_000,
                    "elapsed_ns": local_index * 1_000,
                },
            }
            if request_id != missing_at:
                response["sim_host_timing"] = timing(value)
            stream.write(json.dumps({"data": response}) + "\n")


class TimingTranscriptTests(unittest.TestCase):
    def test_process_restart_starts_new_counter_segment(self) -> None:
        with tempfile.TemporaryDirectory(dir="/tmp") as tmp:
            transcript = Path(tmp) / "transcript.jsonl"
            write_transcript(transcript)
            report = verify_profile.verify_transcript_timings(transcript)

        segments = report["process_instance_segments"]
        self.assertEqual([segment["status_count"] for segment in segments], [721, 2])
        first = segments[0]["cumulative_phase_summary"][PHASES[0]]
        second = segments[1]["cumulative_phase_summary"][PHASES[0]]
        self.assertEqual(first["last_status_cumulative"]["count"], 721)
        self.assertEqual(first["between_first_and_last_status_delta"]["count"], 720)
        self.assertEqual(second["first_status_cumulative"]["count"], 1)
        self.assertEqual(second["last_status_cumulative"]["count"], 2)
        self.assertEqual(report["sim_host_timing_status"], "verified")

    def test_missing_summary_is_unavailable_not_zero(self) -> None:
        with tempfile.TemporaryDirectory(dir="/tmp") as tmp:
            transcript = Path(tmp) / "transcript.jsonl"
            write_transcript(transcript, missing_at=100)
            report = verify_profile.verify_transcript_timings(transcript)

        self.assertEqual(report["sim_host_timing_status"], "unavailable_payload_hash_only")
        self.assertEqual(report["sim_host_timing_missing_status_count"], 1)
        segment = report["process_instance_segments"][0]
        self.assertEqual(segment["cumulative_phase_summary"][PHASES[0]]["last_status_cumulative"]["count"], 721)

    def test_regressed_cumulative_counter_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(dir="/tmp") as tmp:
            transcript = Path(tmp) / "transcript.jsonl"
            write_transcript(transcript, regress_at=100)
            with self.assertRaisesRegex(verify_profile.VerifyError, "timing_counter_regressed"):
                verify_profile.verify_transcript_timings(transcript)


if __name__ == "__main__":
    unittest.main()
