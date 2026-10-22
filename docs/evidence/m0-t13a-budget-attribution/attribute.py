#!/usr/bin/env python3
"""Recompute T13-A normal-budget attribution from retained 32c6 evidence."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "docs/evidence/m0-t13a-extents-joint-retry-02"
WORK = ROOT / "work/m0-t13a-extents-joint-l1-retry-02/observed"


def read_json(path: Path):
    return json.loads(path.read_text())


def frame_total(frame: dict) -> int:
    if not frame["group_capture_complete"]:
        raise ValueError("attempted to total an incomplete process frame")
    return sum(item[2] for item in frame["member_footprints_pid_birth_bytes"])


def frame_roles(frame: dict) -> dict[str, int]:
    by_pid = {item[0]: item[2] for item in frame["member_footprints_pid_birth_bytes"]}
    return {member[4]: by_pid[member[0]] for member in frame["members"]}


def owner_ranges(capture: dict, kind: str) -> list[dict]:
    return [item for item in capture["ranges"] if item["kind"] == kind]


def main() -> None:
    source = read_json(EVIDENCE / "source.json")
    freeze = read_json(EVIDENCE / "freeze.json")
    summary = read_json(EVIDENCE / "measurement-summary.json")
    audit = read_json(EVIDENCE / "address-audit.json")
    rows = [json.loads(line) for line in (WORK / "host-samples.jsonl").read_text().splitlines()]

    expected_sha = "32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2"
    if source["source_sha256"] != expected_sha or freeze["source_sha256"] != expected_sha:
        raise ValueError("source identity mismatch")
    if summary["normal_bytes"] != 3_328_599_654:
        raise ValueError("frozen normal threshold changed")
    if audit["source_sha256"] != expected_sha:
        raise ValueError("VM address audit source mismatch")

    complete = [(index, frame, frame_total(frame)) for index, frame in enumerate(rows)
                if frame["group_capture_complete"]]
    first_over = next(item for item in complete if item[2] > summary["normal_bytes"])
    reduced = [(index, frame, total) for index, frame, total in complete
               if frame.get("r07_applied")
               and frame["r07_applied"]["state"]["replaced_bytes"] > 0]
    # If multiple consecutive frames tie, retain the last one: it is the
    # complete frame immediately before the next measured state transition.
    reduced_peak = max(reduced, key=lambda item: (item[2], item[0]))
    complete_peak = max(complete, key=lambda item: (item[2], item[0]))
    if len(complete) != summary["complete_frames"]:
        raise ValueError("complete-frame count differs from retained summary")

    layout_audit = next(c for c in audit["captures"]
                        if c["role"] == "layout" and c["label"] == "inflight")
    pool_audit = next(c for c in audit["captures"]
                      if c["role"] == "pool" and c["label"] == "inflight")
    arena_ranges = owner_ranges(layout_audit, "role_arena")
    pool_ranges = owner_ranges(pool_audit, "normal_pool_mapped_page")
    if len(arena_ranges) != 1 or len(pool_ranges) != 64:
        raise ValueError("unexpected named owner range shape")
    arena = arena_ranges[0]
    arena_bytes = arena["len"]
    arena_grant = arena["grant_bytes"]
    grant_gap = arena_grant - arena_bytes
    pool_mapped = sum(item["len"] for item in pool_ranges)

    def breakdown(item: tuple[int, dict, int]) -> dict:
        index, frame, total = item
        roles = frame_roles(frame)
        state = frame["r07_applied"]["state"]
        r07_mapped = state["mapped_bytes"]
        r07_replaced = state["replaced_bytes"]
        r07_helper_permitted = frame["r07_applied"]["permitted_bytes"]
        layout_extra = roles["layout-r07-session"] - arena_bytes - r07_mapped
        pool_extra = roles["normal-pool-host"] - pool_mapped
        control = roles["observer"] + roles["r07-layout-driver"]
        overhead = layout_extra + pool_extra + control
        return {
            "frame_index_zero_based": index,
            "jsonl_line_one_based": index + 1,
            "complete": frame["group_capture_complete"],
            "process_footprints_bytes": roles,
            "group_footprint_bytes": total,
            "normal_excess_bytes": total - summary["normal_bytes"],
            "r07_mapped_bytes": r07_mapped,
            "r07_replaced_bytes": r07_replaced,
            "r07_helper_permitted_bytes_not_subtracted": r07_helper_permitted,
            "layout_footprint_outside_arena_and_r07_bytes": layout_extra,
            "pool_footprint_outside_mapping_bytes": pool_extra,
            "observer_plus_driver_footprint_bytes": control,
            "sum_observed_overhead_bytes": overhead,
            "allocator_grant_minus_arena_bytes": grant_gap,
            "excess_identity_bytes": overhead - grant_gap - r07_replaced,
        }

    first = breakdown(first_over)
    reduced_peak_result = breakdown(reduced_peak)
    peak = breakdown(complete_peak)

    r07_full = peak["r07_mapped_bytes"]
    normal_rebuilt = arena_grant + r07_full + pool_mapped
    if normal_rebuilt != summary["normal_bytes"]:
        raise ValueError("named fixed grants no longer reconcile to normal")
    if first["excess_identity_bytes"] != first["normal_excess_bytes"]:
        raise ValueError("first-over component identity failed")
    if reduced_peak_result["excess_identity_bytes"] != reduced_peak_result["normal_excess_bytes"]:
        raise ValueError("reduced-R07 peak component identity failed")
    if peak["excess_identity_bytes"] != peak["normal_excess_bytes"]:
        raise ValueError("full-R07 peak component identity failed")
    if peak["group_footprint_bytes"] != summary["complete_frame_peak_footprint_bytes"]:
        raise ValueError("peak differs from retained measurement summary")

    result = {
        "schema": "mycity-t13a-normal-budget-attribution-v1",
        "source_sha256": expected_sha,
        "inputs": {
            "raw_frames": "work/m0-t13a-extents-joint-l1-retry-02/observed/host-samples.jsonl",
            "address_audit": "docs/evidence/m0-t13a-extents-joint-retry-02/address-audit.json",
            "measurement_summary": "docs/evidence/m0-t13a-extents-joint-retry-02/measurement-summary.json",
        },
        "fixed_named_grants": {
            "layout_role_arena_address_bytes": arena_bytes,
            "layout_allocator_grant_bytes": arena_grant,
            "allocator_bootstrap_plus_header_bytes": grant_gap,
            "r07_full_mapped_bytes": r07_full,
            "normal_pool_mapped_bytes": pool_mapped,
            "reconstructed_normal_bytes": normal_rebuilt,
        },
        "first_complete_frame_over_normal": first,
        "maximum_complete_frame_with_r07_replaced_bytes": reduced_peak_result,
        "maximum_complete_frame": peak,
        "full_vs_reduced_r07_peak": {
            "layout_process_footprint_increase_bytes": (
                peak["process_footprints_bytes"]["layout-r07-session"]
                - reduced_peak_result["process_footprints_bytes"]["layout-r07-session"]
            ),
            "r07_mapping_restored_bytes": (
                peak["r07_mapped_bytes"] - reduced_peak_result["r07_mapped_bytes"]
            ),
            "difference_bytes": (
                peak["process_footprints_bytes"]["layout-r07-session"]
                - reduced_peak_result["process_footprints_bytes"]["layout-r07-session"]
                - (peak["r07_mapped_bytes"] - reduced_peak_result["r07_mapped_bytes"])
            ),
        },
        "interpretation": {
            "vm_address_coverage_is_physical_credit": False,
            "helper_permitted_bytes_are_subtracted": False,
            "payload_or_normal_threshold_changed": False,
            "safe_reduction_claimed": False,
        },
    }
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
