#!/usr/bin/env python3
"""Lossless 8h capacity screen from all 4Hz frames and fixed-work metrics."""
import json
import math
import sys
from pathlib import Path

from frame_codec import BLOCK_FRAMES, MAGIC, iter_blocks
from preflight import EXPECTED_SOURCE
from source_identity import load_fixed_source_document

LOG_CAP_BYTES = 256 * 1024**2
SAMPLE_HZ = 4
SMOKE_SECONDS = 600
FORMAL_SECONDS = 28_800
FORMAL_SCANS = 28_800


def project(frame_count, raw_bytes, encoded_blocks, business_metric_bytes,
            layout_metric_bytes, layout_scans, target_seconds=FORMAL_SECONDS,
            target_scans=FORMAL_SCANS):
    if type(frame_count) is not int or frame_count <= 0 or type(raw_bytes) is not int or raw_bytes <= 0:
        raise ValueError("smoke must contain complete positive-length frames")
    if not encoded_blocks or any(type(n) is not int or n <= 0 for n in encoded_blocks):
        raise ValueError("smoke must contain valid independently encoded blocks")
    for name, value in (("business", business_metric_bytes), ("layout", layout_metric_bytes), ("layout scans", layout_scans)):
        if type(value) is not int or value < 0:
            raise ValueError(name + " must be a nonnegative integer")
    if layout_scans <= 0 or target_seconds <= 0 or target_scans <= 0:
        raise ValueError("target durations/scans must be positive")
    expected_frames = math.ceil(target_seconds * SAMPLE_HZ)
    block_count = math.ceil(expected_frames / BLOCK_FRAMES)
    max_block = max(encoded_blocks)
    host_upper = len(MAGIC) + max_block * block_count
    # Business jobs (720h/723 targets/2880 queries/96 saves) are fixed in both
    # cadences. Only layout compute-duration records follow the scan count.
    layout_projection = math.ceil(layout_metric_bytes * target_scans / layout_scans)
    combined = host_upper + business_metric_bytes + layout_projection
    return {
        "smoke_frame_count": frame_count,
        "smoke_raw_host_log_bytes": raw_bytes,
        "smoke_encoded_block_count": len(encoded_blocks),
        "smoke_max_encoded_4hz_block_bytes": max_block,
        "sample_hz_unchanged": SAMPLE_HZ,
        "formal_seconds": target_seconds,
        "formal_expected_frame_count": expected_frames,
        "formal_expected_block_count": block_count,
        "formal_host_log_upper_estimate_bytes": host_upper,
        "formal_host_log_stream_header_bytes": len(MAGIC),
        "smoke_business_compute_durations_bytes": business_metric_bytes,
        "compute_duration_record_bytes": 9,
        "smoke_business_compute_duration_records": business_metric_bytes // 9,
        "formal_business_compute_durations_estimate_bytes": business_metric_bytes,
        "formal_business_compute_duration_records_estimate": business_metric_bytes // 9,
        "smoke_layout_compute_durations_bytes": layout_metric_bytes,
        "smoke_layout_compute_duration_records": layout_metric_bytes // 9,
        "smoke_layout_scans": layout_scans,
        "formal_layout_scans": target_scans,
        "formal_layout_compute_durations_estimate_bytes": layout_projection,
        "formal_layout_compute_duration_records_estimate": layout_projection // 9,
        "formal_combined_log_upper_estimate_bytes": combined,
        "hard_cap_bytes": LOG_CAP_BYTES,
        "cap_estimate": "conservative_projection_under_hard_cap" if combined <= LOG_CAP_BYTES else "conservative_projection_exceeds_hard_cap",
        "method": "all full-information 4Hz frames are encoded as independent lossless four-frame DEFLATE blocks; maximum observed encoded block times formal block count; fixed business task output is not time-scaled; layout compute-duration records scale by scan count only",
        "metric_source_basis": "fixed ZIP crates/city-headless/src/scale_metrics.rs writes each compute-duration as 9 bytes (class u8 + duration_ns u64 LE); scale_endurance.rs holds the same 720-hour business tasks for smoke/formal; joint_driver.rs runs 600 smoke layout scans and the frozen contract runs 28800 formal scans",
        "limitations": "smoke-derived maximum-block upper screen, not proof of formal actual size; live 256MiB combined cap remains mandatory; no frame, latency, or pressure information is dropped",
    }


def analyze(run_root):
    run_root = Path(run_root).resolve(strict=True)
    observed = run_root / "observed"
    wrapper = json.loads((observed / "observer-report.json").read_text())
    model = json.loads((observed / "model/report.json").read_text())
    source = load_fixed_source_document(Path(__file__).resolve().parent / "source.json", EXPECTED_SOURCE)
    if wrapper.get("status") != "passed_measurement_wrapper" or wrapper.get("model_exit_code") != 0:
        raise ValueError("only a completed joint smoke can support a formal capacity screen")
    if model.get("schema") != "mycity-joint-endurance-v1" or model.get("cadence") != "smoke":
        raise ValueError("source/model report is not the same-source joint smoke")
    if model.get("source_sha256") != source["fixed_source_sha256"]:
        raise ValueError("smoke report does not match the fixed frozen source")
    sample_path = observed / wrapper.get("sample_log_file", "host-samples.mcf")
    if sample_path.stat().st_size != wrapper.get("sample_log_bytes"):
        raise ValueError("observer summary and encoded host log size disagree")
    raw_frames = []
    block_lengths = []
    with sample_path.open("rb") as stream:
        for block in iter_blocks(stream):
            block_lengths.append(block["encoded_bytes"])
            raw_frames.extend(block["frames"])
    if len(raw_frames) != wrapper.get("samples"):
        raise ValueError("observer summary and decoded frame count disagree")
    raw_bytes = sum(map(len, raw_frames))
    if raw_bytes != wrapper.get("sample_log_raw_jsonl_bytes"):
        raise ValueError("observer raw-byte count and decoded frames disagree")
    frames = [json.loads(line) for line in raw_frames]
    layout = json.loads((observed / "model/layout/report.json").read_text())
    business = json.loads((observed / "model/business/report.json").read_text())
    metrics = {"business": 0, "layout": 0}
    metric_files = []
    for path in sorted(observed.rglob("compute-durations.bin")):
        relative = path.relative_to(observed)
        if relative.parts[0] == "model" and relative.parts[1] in metrics:
            category = relative.parts[1]
        else:
            raise ValueError("unclassified compute-durations file: " + str(relative))
        size = path.stat().st_size
        if size % 9:
            raise ValueError("compute-durations file has partial 9-byte record: " + str(relative))
        metrics[category] += size
        metric_files.append({"path": str(path), "category": category, "bytes": size, "records": size // 9})
    if business.get("clock", {}).get("hours") != 720 or business.get("clock", {}).get("targets") != 723 or business.get("clock", {}).get("queries_completed") != 2880 or business.get("clock", {}).get("saves_completed") != 96:
        raise ValueError("smoke business work counts do not match the fixed 720h contract")
    layout_scans = layout.get("scans_completed", layout.get("published_scans_completed", layout.get("scans")))
    if layout_scans is None:
        layout_scans = model.get("layout_scans")
    result = project(len(frames), raw_bytes, block_lengths, metrics["business"], metrics["layout"], int(layout_scans))
    incomplete = [i for i, frame in enumerate(frames) if frame.get("group_capture_complete") is not True]
    result.update({
        "smoke_observed_wall_seconds": wrapper["actual_wall_seconds"],
        "smoke_business_wall_seconds": business["clock"].get("actual_wall_seconds"),
        "smoke_business_required_wall_seconds": business["clock"].get("required_wall_seconds"),
        "smoke_layout_work_wall_seconds": layout.get("work_wall_seconds"),
        "observer_source_sha256": wrapper.get("observer_source_sha256"),
        "native_metrics_source_sha256": wrapper.get("native_metrics_source_sha256"),
        "incomplete_group_frame_count": len(incomplete),
        "incomplete_group_frame_indices_zero_based": incomplete,
        "incomplete_frames_are_retained_in_capacity_projection": True,
        "physical_identity_completeness": "unaccepted; unknowns and misses retained in full frames; this flag is not an added functional endurance gate",
        "functional_formal_eligibility": "eligible_for_functional_8h_trial_only_if_capacity_screen_under_cap_and_same-source checks pass" if result["cap_estimate"] == "conservative_projection_under_hard_cap" else "blocked_by_conservative_capacity_screen",
        "physical_performance_and_full_m0": "unaccepted",
        "external_compiler_frames": sum(bool(frame.get("external_compilers")) for frame in frames),
        "raw_frame_source": str(sample_path),
        "compute_duration_files": metric_files,
        "business_work_counts": business["clock"],
        "layout_scan_count_source": "layout report when present, otherwise joint report",
        "analysis_role": "offline capacity analysis; does not revise or certify the historical 02 run",
    })
    return result


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: log_budget.py COMPLETED_SMOKE_WORK_ROOT")
    result = analyze(sys.argv[1])
    output = Path(sys.argv[1]) / "log-budget.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
