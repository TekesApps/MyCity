#!/usr/bin/env python3
"""Offline, read-only diagnosis and lossless recoding of preserved attempt 02."""
import hashlib
import json
import os
import shutil
import tempfile
from collections import Counter
from pathlib import Path

from frame_codec import FrameWriter, iter_blocks
from log_budget import LOG_CAP_BYTES
from log_budget import analyze

REPO = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OLD = REPO / "work/j2s-01/observed"
OUT = HERE / "analysis"


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def main():
    OUT.mkdir(exist_ok=True)
    raw_path = OLD / "host-samples.jsonl"
    raw = raw_path.read_bytes()
    lines = raw.splitlines(keepends=True)
    frames = [json.loads(line) for line in lines]
    compressed = OUT / "attempt-02-lossless-host-samples.mcf"
    with compressed.open("wb") as stream:
        writer = FrameWriter(stream, LOG_CAP_BYTES, lambda: 0)
        for line in lines:
            writer.add(line)
        writer.flush()
    decoded = []
    block_sizes = []
    with compressed.open("rb") as stream:
        for block in iter_blocks(stream):
            decoded.extend(block["frames"])
            block_sizes.append(block["encoded_bytes"])
    if decoded != lines or b"".join(decoded) != raw:
        raise RuntimeError("lossless recode did not reproduce exact original JSONL bytes")

    heap_rows = [json.loads(line) for line in (OLD / "model/heaps.jsonl").read_text().splitlines() if line]
    heap_roles = {row["pid"]: row.get("role") for row in heap_rows if isinstance(row.get("pid"), int)}
    pid_member_frames = {}
    for idx, frame in enumerate(frames):
        for member in frame.get("members", []):
            pid_member_frames.setdefault(member[0], []).append(idx)
    incomplete = []
    enumeration_pids = set()
    identity_pids = set()
    enumeration_run_heap = set()
    identity_run_heap = set()
    for idx, frame in enumerate(frames):
        if frame.get("group_capture_complete") is True:
            continue
        enums = frame.get("enumeration_failures", [])
        identities = frame.get("identity_uncertainties", [])
        for item in enums:
            pid = item["pid"]
            enumeration_pids.add(pid)
            if pid in heap_roles:
                enumeration_run_heap.add(pid)
        for item in identities:
            pid = item["pid"]
            identity_pids.add(pid)
            if pid in heap_roles:
                identity_run_heap.add(pid)
        incomplete.append({
            "frame_index_zero_based": idx,
            "t_seconds": frame.get("t"),
            "enumeration_failures": enums,
            "identity_uncertainties": identities,
            "pending_identities": frame.get("pending_identities", []),
            "missing_selected_processes": frame.get("missing_selected_processes", []),
            "missing_footprint": frame.get("missing_footprint"),
            "missing_rss": frame.get("missing_rss"),
        })
    enum_rows = []
    ident_rows = []
    identity_member_roles = Counter()
    for frame in incomplete:
        source_frame = frames[frame["frame_index_zero_based"]]
        roles_by_pid = {member[0]: member[-1] for member in source_frame.get("members", [])}
        for err in frame["enumeration_failures"]:
            pid = err["pid"]
            enum_rows.append({**err, "frame_index_zero_based": frame["frame_index_zero_based"], "model_heap_role": heap_roles.get(pid), "selected_member_role": roles_by_pid.get(pid)})
        for err in frame["identity_uncertainties"]:
            pid = err["pid"]
            prior = [i for i in pid_member_frames.get(pid, []) if i < frame["frame_index_zero_based"]]
            identity_member_roles[roles_by_pid.get(pid, "not_listed")] += 1
            ident_rows.append({**err, "frame_index_zero_based": frame["frame_index_zero_based"], "model_heap_role": heap_roles.get(pid), "selected_member_role": roles_by_pid.get(pid), "prior_sampled_member_frame": prior[-1] if prior else None})

    wrapper = json.loads((OLD / "observer-report.json").read_text())
    model = json.loads((OLD / "model/report.json").read_text())
    with tempfile.TemporaryDirectory(prefix="m0-t13a-03-offline-") as temp:
        temp_obs = Path(temp) / "observed"
        model_dir = temp_obs / "model"
        model_dir.mkdir(parents=True)
        shutil.copyfile(compressed, temp_obs / "host-samples.mcf")
        shutil.copyfile(OLD / "observer-report.json", temp_obs / "observer-report.json")
        for relative in ("report.json", "business/report.json", "layout/report.json"):
            destination = model_dir / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            os.symlink(OLD / "model" / relative, destination)
        for old_file in (OLD / "model").rglob("compute-durations.bin"):
            relative = old_file.relative_to(OLD / "model")
            destination = model_dir / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            os.symlink(old_file, destination)
        adjusted = dict(wrapper, sample_log_file="host-samples.mcf", sample_log_bytes=compressed.stat().st_size, sample_log_raw_jsonl_bytes=len(raw))
        (temp_obs / "observer-report.json").write_text(json.dumps(adjusted))
        smoke_root = temp_obs.parent
        budget = analyze(smoke_root)
    budget["raw_frame_source"] = str(raw_path)
    budget["lossless_recode"] = {
        "encoded_file": str(compressed),
        "original_sha256": sha(raw_path),
        "encoded_sha256": sha(compressed),
        "original_jsonl_bytes": len(raw),
        "compressed_mcf_bytes": compressed.stat().st_size,
        "round_trip_exact_byte_equality": True,
        "block_count": len(block_sizes),
        "max_encoded_block_bytes": max(block_sizes),
        "frames_per_block": 4,
    }
    budget["old_02_projection"] = json.loads((REPO / "docs/evidence/m0-t13a-joint-smoke-control-02/results/log-budget-diagnostic.json").read_text())
    (OUT / "attempt-02-log-capacity-analysis.json").write_text(json.dumps(budget, indent=2) + "\n")
    issue_report = {
        "schema": "m0-t13a-attempt-02-frame-diagnosis-v1",
        "original_log": str(raw_path),
        "original_log_sha256": sha(raw_path),
        "frames": len(frames),
        "original_raw_log_bytes": len(raw),
        "maximum_observed_sample_gap_seconds": max(b["t"] - a["t"] for a, b in zip(frames, frames[1:])),
        "incomplete_frame_count": len(incomplete),
        "incomplete_frame_indices_zero_based": [row["frame_index_zero_based"] for row in incomplete],
        "incomplete_frames_are_log_rows_not_omitted_lines": True,
        "enumeration_failure_count": sum(len(x["enumeration_failures"]) for x in incomplete),
        "unique_enumeration_failure_pids": len(enumeration_pids),
        "enumeration_failures_confirmed_absent_after_query": sum(e.get("confirmed_absent_after_query") is True for e in enum_rows),
        "enumeration_failures_not_confirmed_absent_after_query": sum(e.get("confirmed_absent_after_query") is False for e in enum_rows),
        "enumeration_failures_matching_model_heap_identity": sorted(enumeration_run_heap),
        "enumeration_failure_pids_without_model_heap_match": sorted(enumeration_pids - enumeration_run_heap),
        "identity_uncertainty_count": sum(len(x["identity_uncertainties"]) for x in incomplete),
        "unique_identity_uncertainty_pids": len(identity_pids),
        "identity_uncertainty_selected_member_role_counts": dict(identity_member_roles),
        "identity_uncertainties_matching_model_heap_identity": sorted(identity_run_heap),
        "identity_uncertainty_pids_without_model_heap_match": sorted(identity_pids - identity_run_heap),
        "enumeration_failure_details": enum_rows,
        "identity_uncertainty_details": ident_rows,
        "overlapping_error_frames": [x["frame_index_zero_based"] for x in incomplete if x["enumeration_failures"] and x["identity_uncertainties"]],
        "interpretation": {
            "raw_log_rows": "all 2441 full JSON frames exist and parse; 44 rows carry incomplete identity metadata; this is not 44 absent log rows",
            "global_process_table_races": "28 ESRCH enumeration failures are preserved; 8 pids have model heap identity and 20 have no run-specific heap match; even confirmed-gone follow-up does not erase the table race",
            "selected_model_member_races": "13 compute-worker process identity uncertainties, 5 ps helper uncertainties, and 2 root exit uncertainties remain explicit; prior sampled membership is reported per PID; no unavailable footprint or identity is treated as zero",
            "short_helpers_and_shutdown": "ps helper rows are short-lived descendants selected from the model tree; business/layout exit races remain identity uncertainties despite adjacent lifecycle evidence",
        },
    }
    (OUT / "attempt-02-frame-diagnosis.json").write_text(json.dumps(issue_report, indent=2) + "\n")
    print(json.dumps({"capacity": {k: budget[k] for k in ("formal_host_log_upper_estimate_bytes", "formal_business_compute_durations_estimate_bytes", "formal_layout_compute_durations_estimate_bytes", "formal_combined_log_upper_estimate_bytes", "cap_estimate", "incomplete_group_frame_count", "external_compiler_frames")}, "round_trip": budget["lossless_recode"], "diagnosis": {k: issue_report[k] for k in ("frames", "incomplete_frame_count", "enumeration_failure_count", "identity_uncertainty_count", "enumeration_failures_matching_model_heap_identity", "identity_uncertainties_matching_model_heap_identity")}}, indent=2))


if __name__ == "__main__":
    main()
