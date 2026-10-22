#!/usr/bin/env python3
"""Conservatively project unchanged 4 Hz full-frame logging onto 8 hours."""
import json
import math
import sys
from pathlib import Path

LOG_CAP_BYTES = 256 * 1024**2
SAMPLE_HZ = 4
SMOKE_SECONDS = 600
FORMAL_SECONDS = 28_800


def forecast(frame_lengths, latency_bytes, target_seconds=FORMAL_SECONDS):
    if not frame_lengths or any(type(n) is not int or n <= 0 for n in frame_lengths):
        raise ValueError("smoke must contain positive-length complete frame lines")
    if type(latency_bytes) is not int or latency_bytes < 0:
        raise ValueError("latency byte count must be a nonnegative integer")
    target_samples = math.ceil(target_seconds * SAMPLE_HZ)
    max_frame = max(frame_lengths)
    average_frame = sum(frame_lengths) / len(frame_lengths)
    fixed_sample_projection = max_frame * target_samples
    latency_projection = math.ceil(latency_bytes * target_seconds / SMOKE_SECONDS)
    projected_combined = fixed_sample_projection + latency_projection
    return {
        "smoke_frames": len(frame_lengths),
        "smoke_frame_bytes": sum(frame_lengths),
        "smoke_frame_average_bytes": average_frame,
        "smoke_frame_max_bytes": max_frame,
        "smoke_frame_p99_bytes": sorted(frame_lengths)[min(len(frame_lengths) - 1, math.ceil(len(frame_lengths) * 0.99) - 1)],
        "sample_hz_unchanged": SAMPLE_HZ,
        "formal_seconds": target_seconds,
        "formal_expected_frame_count": target_samples,
        "formal_host_log_upper_estimate_bytes": fixed_sample_projection,
        "smoke_compute_durations_bytes": latency_bytes,
        "formal_compute_durations_estimate_bytes": latency_projection,
        "formal_combined_log_upper_estimate_bytes": projected_combined,
        "hard_cap_bytes": LOG_CAP_BYTES,
        "cap_estimate": "conservative_projection_under_hard_cap" if projected_combined <= LOG_CAP_BYTES else "conservative_projection_exceeds_hard_cap",
        "method": "max complete smoke frame length times formal 4Hz frame count, plus compute-durations bytes scaled by formal/smoke layout seconds; no sampling or information reduction",
        "acceptance": "upper_bound_screen_only; a projection above the cap does not establish that an actual formal run would exceed it; only a completed formal observer report proves the live cap held",
    }


def analyze(run_root):
    run_root = Path(run_root).resolve(strict=True)
    observed = run_root / "observed"
    wrapper = json.loads((observed / "observer-report.json").read_text())
    model = json.loads((observed / "model/report.json").read_text())
    source = json.loads((Path(__file__).resolve().parent / "source.json").read_text())
    if wrapper.get("status") != "passed_measurement_wrapper" or wrapper.get("model_exit_code") != 0:
        raise ValueError("only a completed joint smoke can support a formal log estimate")
    if model.get("schema") != "mycity-joint-endurance-v1" or model.get("cadence") != "smoke":
        raise ValueError("source/model report is not the same-source joint smoke")
    if model.get("source_sha256") != source["fixed_source_sha256"]:
        raise ValueError("smoke report does not match the fixed frozen source")
    sample_path = observed / "host-samples.jsonl"
    raw = sample_path.read_bytes()
    if not raw.endswith(b"\n"):
        raise ValueError("host sample log has an incomplete final frame")
    lines = raw.splitlines(keepends=True)
    frames = [json.loads(line) for line in lines]
    if len(frames) != wrapper.get("samples") or len(raw) != wrapper.get("sample_log_bytes"):
        raise ValueError("observer summary and raw full-frame log disagree")
    if len(frames) < SAMPLE_HZ * SMOKE_SECONDS:
        raise ValueError("smoke did not retain at least 600 seconds of 4Hz frames")
    if not all(frame.get("group_capture_complete") is True for frame in frames):
        raise ValueError("smoke contains incomplete group frames; preserve and report them")
    latency_bytes = sum(p.stat().st_size for p in observed.rglob("compute-durations.bin"))
    result = forecast([len(line) for line in lines], latency_bytes)
    result.update({
        "smoke_observed_wall_seconds": wrapper["actual_wall_seconds"],
        "observer_source_sha256": wrapper.get("observer_source_sha256"),
        "native_metrics_source_sha256": wrapper.get("native_metrics_source_sha256"),
        "all_smoke_group_frames_complete": True,
        "external_compiler_frames": sum(bool(frame.get("external_compilers")) for frame in frames),
        "raw_host_sample_log": str(sample_path),
        "compute_duration_files": [
            {"path": str(p), "bytes": p.stat().st_size}
            for p in observed.rglob("compute-durations.bin")
        ],
    })
    return result


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: log_budget.py COMPLETED_SMOKE_WORK_ROOT")
    result = analyze(sys.argv[1])
    output = Path(sys.argv[1]) / "log-budget.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
