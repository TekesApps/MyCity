#!/usr/bin/env python3
"""Recompute allocator lifetime peak and snapshot-phase minima from raw evidence."""

from __future__ import annotations

import hashlib
import json
import struct
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
FREEZE = ROOT / "docs/evidence/m0-t13a-extents-joint-retry-02"
RUN = ROOT / "work/m0-t13a-extents-joint-l1-retry-02/observed/model"
SOURCE = "32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2"
ZIP_SHA = "84c450c2c866578b02205030c7587df1dbb719660422fe64865ae62fe2ce908d"
EXPECTED_RAW = {
    "heaps": "observed/model/heaps.jsonl",
    "generation_1": "observed/model/layout/snapshots/snapshot-000001/allocation-phases.bin",
    "generation_2": "observed/model/layout/snapshots/snapshot-000002/allocation-phases.bin",
}
SOURCE_FILES = [
    "crates/city-headless/src/layout_load.rs",
    "crates/city-headless/src/layout_load/decode_workspace.rs",
    "crates/city-headless/src/layout_load/r07.rs",
    "crates/city-role-allocator/src/adapter.rs",
]
TRACE_FIELDS = [
    "magic_version", "sequence", "generation", "phase", "index",
    "request_bytes", "current_requested_bytes", "arena_used_bytes",
    "arena_free_bytes", "allocation_failures", "reserve_available_pages",
    "reserve_total_loans", "decode_loans", "elapsed_ns", "reserved_0",
    "reserved_1",
]


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def stream_source_sha256(entries: list[tuple[str, bytes]]) -> str:
    """Same sorted-name, LE-u64 length stream used by the frozen harness."""
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda item: item[0]):
        name_bytes = name.encode()
        digest.update(struct.pack("<Q", len(name_bytes)))
        digest.update(name_bytes)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def file_record(path: Path, repo_relative: str) -> dict:
    if not path.is_file():
        raise FileNotFoundError(path)
    return {
        "path": repo_relative,
        "absolute_path": str(path),
        "status": "retained_raw_file",
        "size_bytes": path.stat().st_size,
        "sha256": sha256(path),
    }


def verify_frozen_source() -> dict:
    source_info = json.loads((FREEZE / "source.json").read_text())
    freeze_info = json.loads((FREEZE / "freeze.json").read_text())
    archive_path = FREEZE / "frozen-source.zip"
    if source_info["source_sha256"] != SOURCE or freeze_info["source_sha256"] != SOURCE:
        raise ValueError("frozen source identity mismatch")
    archive_sha = sha256(archive_path)
    if archive_sha != ZIP_SHA or freeze_info["source_zip_sha256"] != archive_sha:
        raise ValueError("frozen source archive SHA mismatch")
    with zipfile.ZipFile(archive_path) as archive:
        if archive.testzip() is not None:
            raise ValueError("frozen source ZIP CRC failure")
        names = archive.namelist()
        if len(names) != 231 or len(names) != source_info["source_files"]:
            raise ValueError("frozen source file count mismatch")
        entries = [(name, archive.read(name)) for name in names]
    stream_sha = stream_source_sha256(entries)
    if stream_sha != SOURCE:
        raise ValueError("frozen source stream SHA mismatch")
    by_name = dict(entries)
    return {
        "source_sha256": SOURCE,
        "source_zip_path": "docs/evidence/m0-t13a-extents-joint-retry-02/frozen-source.zip",
        "source_zip_sha256": archive_sha,
        "source_stream_sha256_recomputed": stream_sha,
        "source_entry_count": len(entries),
        "files": {
            name: {"sha256": hashlib.sha256(by_name[name]).hexdigest(), "bytes": len(by_name[name])}
            for name in SOURCE_FILES
        },
        "source_line_references": {
            "layout_load.rs": {
                "lines": ["22-26", "326-394", "426-449", "471-493", "497-539", "722-750", "796-806", "907-954", "978-1005"],
                "meaning": "fixed normal/work/COW and metadata budgets; allocator grant; reserve/page ownership; snapshot clones and stack; reusable buffers; streamed work reserve and decode; trace phase 5 table decode",
            },
            "decode_workspace.rs": {
                "lines": ["3-39", "54-69", "90-92"],
                "meaning": "64MiB initialized decoder workspace, validated table copy-back, metadata/stack accounting note",
            },
            "r07.rs": {
                "lines": ["129-145", "274-281"],
                "meaning": "extent metadata capacity stays at logical quantum slot cap; control-byte charge follows that capacity",
            },
            "adapter.rs": {
                "lines": ["105-165"],
                "meaning": "fixed System-backed arena is fully initialized and retained for process lifetime",
            },
        },
    }


def trace_summary(path: Path, generation: int, repo_relative: str) -> dict:
    raw = path.read_bytes()
    record_size = 128
    if len(raw) == 0 or len(raw) % record_size:
        raise ValueError(f"invalid trace size: {path}")
    rows = [struct.unpack("<16Q", raw[i:i + record_size]) for i in range(0, len(raw), record_size)]
    for position, row in enumerate(rows):
        if row[0] != 0x4D59435452414301:
            raise ValueError(f"bad trace magic at record {position}")
        if row[1] != position:
            raise ValueError(f"non-contiguous sequence at record {position}")
        if row[2] != generation:
            raise ValueError(f"generation mismatch at record {position}")
    min_row = min(rows, key=lambda row: row[8])
    if min_row[12] > min_row[11]:
        raise ValueError("decode loans exceed total reserve loans")
    return {
        **file_record(path, repo_relative),
        "record_size_bytes": record_size,
        "record_count": len(rows),
        "field_order": TRACE_FIELDS,
        "minimum_free": {
            "sequence": min_row[1],
            "generation": min_row[2],
            "phase": min_row[3],
            "phase_name": "table_decode",
            "index": min_row[4],
            "request_bytes": min_row[5],
            "current_requested_bytes": min_row[6],
            "arena_used_bytes": min_row[7],
            "arena_free_bytes": min_row[8],
            "allocation_failures": min_row[9],
            "reserve_available_pages": min_row[10],
            "reserve_total_loans": min_row[11],
            "decode_loans": min_row[12],
            "cow_loans": min_row[11] - min_row[12],
            "elapsed_ns": min_row[13],
        },
        "phase_counts": {
            str(phase): sum(row[3] == phase for row in rows)
            for phase in sorted({row[3] for row in rows})
        },
    }


def main() -> None:
    source = verify_frozen_source()
    heaps_path = RUN / "heaps.jsonl"
    heap_rows = [json.loads(line) for line in heaps_path.read_text().splitlines() if line.strip()]
    layouts = [
        row for row in heap_rows
        if row.get("role") == "layout-r07-session"
        and row.get("source_sha256") == SOURCE
        and row.get("success") is True
    ]
    if len(layouts) != 1:
        raise ValueError(f"expected exactly one successful frozen layout heap record, got {len(layouts)}")
    row = layouts[0]
    heap = row["heap"]
    arena = row["role_arena"]["activation"]
    peak = heap["lifetime_peak_requested_bytes"]
    if arena["grant_bytes"] != 2_983_880_294 or arena["arena_bytes"] != 2_983_813_465:
        raise ValueError("layout arena grant/size differs from frozen measurement")
    if peak != 2_982_633_942:
        raise ValueError("heap lifetime peak differs from frozen measurement")

    files = {
        "heaps": file_record(heaps_path, "work/m0-t13a-extents-joint-l1-retry-02/observed/model/heaps.jsonl"),
        "generation_1": trace_summary(
            RUN / "layout/snapshots/snapshot-000001/allocation-phases.bin", 1,
            "work/m0-t13a-extents-joint-l1-retry-02/observed/model/layout/snapshots/snapshot-000001/allocation-phases.bin",
        ),
        "generation_2": trace_summary(
            RUN / "layout/snapshots/snapshot-000002/allocation-phases.bin", 2,
            "work/m0-t13a-extents-joint-l1-retry-02/observed/model/layout/snapshots/snapshot-000002/allocation-phases.bin",
        ),
    }
    if files["generation_1"]["record_count"] != 61 or files["generation_2"]["record_count"] != 61:
        raise ValueError("allocation trace record count changed")
    expected_minima = {
        "generation_1": (42, 1_351_912, 58, 70, 64, 6),
        "generation_2": (57, 1_327_024, 56, 72, 64, 8),
    }
    for name, (sequence, free, reserve, loans, decode, cow) in expected_minima.items():
        actual = files[name]["minimum_free"]
        values = (actual["sequence"], actual["arena_free_bytes"], actual["reserve_available_pages"], actual["reserve_total_loans"], actual["decode_loans"], actual["cow_loans"])
        if values != (sequence, free, reserve, loans, decode, cow):
            raise ValueError(f"{name} minimum-free row mismatch: {values}")

    result = {
        "schema": "mycity-t13a-allocation-capacity-v1",
        "source": source,
        "measurement_scope": {
            "run": "m0-t13a-extents-joint-l1-retry-02",
            "raw_files_status": "retained_raw_file; no claim that snapshot files are a direct restore point",
            "workload_or_acceptance_changed": False,
            "cargo_or_model_run_performed_by_this_verifier": False,
            "normal_threshold_or_payload_changed": False,
        },
        "layout_heap": {
            "raw_record": files["heaps"],
            "pid": row["pid"],
            "instance_unix_ns": row["instance_unix_ns"],
            "role": row["role"],
            "successful": row["success"],
            "grant_bytes": arena["grant_bytes"],
            "arena_bytes": arena["arena_bytes"],
            "bootstrap_charged_bytes": arena["bootstrap_charged_bytes"],
            "adapter_header_bytes": arena["adapter_header_bytes"],
            "initialized_bytes": arena["initialized_bytes"],
            "lifetime_peak_requested_bytes": peak,
            "arena_bytes_minus_heap_peak_bytes": arena["arena_bytes"] - peak,
            "scope_note": row["scope"],
        },
        "snapshot_allocation_phases": {
            "record_layout": "128-byte little-endian records, <16Q, as defined in frozen layout_load.rs:812-855",
            "phase_5": "table_decode; confirmed at frozen layout_load.rs:981-987",
            "generations": {"1": files["generation_1"], "2": files["generation_2"]},
        },
        "retention": {
            "original_raw_heap_and_phase_files_kept": True,
            "retired_or_compressed_inputs_used": False,
            "direct_restore_claim": False,
            "exact_files": {name: files[name] for name in EXPECTED_RAW},
        },
        "interpretation": {
            "proven_safe_reduction_bytes": 0,
            "metadata_charge_or_work_payload_trimmed": False,
            "conclusion": "The frozen allocation relationships explain why the existing arenas are live, but these records do not identify a safely removable allocation. Keep metadata, WORK_PAGES, R07 payload and normal threshold unchanged pending a new synchronized allocation/VM-map attribution that isolates a dead or duplicate backing.",
        },
    }
    output = HERE / "allocation-capacity.json"
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"wrote {output}")


if __name__ == "__main__":
    main()
