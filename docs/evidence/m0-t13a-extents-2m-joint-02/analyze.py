"""Address intersections and R07 logical topology; never derive residual credits."""
import hashlib
import json
import re
from collections import Counter
from pathlib import Path

from topology import assert_pinned_identity, assert_repaid_identity, r07_extents

D = Path(__file__).resolve().parent
R = D.parents[2]
terminal = json.loads((D / "terminal.json").read_text())
cold = json.loads((D / "cold-result.json").read_text())
reports = []

assert terminal["source_sha256"] == cold["source_sha256"]
assert cold["all_generations_cold_restored"]
assert [item["generation"] for item in cold["generations"]] == [1, 2]
assert all(item["exit"] == 0 for item in cold["generations"])

for capture in terminal["captures"]:
    text = (R / capture["stdout"]).read_text()
    page_match = re.search(r"VM page size:\s+(\d+) bytes", text)
    assert page_match, capture["stdout"]
    page = int(page_match.group(1))
    regions = []
    for line in text.splitlines():
        match = re.match(
            r"(.+?)\s+([0-9a-f]+)-([0-9a-f]+)\s+\[\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*\]",
            line,
        )
        if match:
            regions.append(
                dict(
                    type=match[1].strip(),
                    start=int(match[2], 16),
                    end=int(match[3], 16),
                    reported_pages=dict(
                        zip(["virtual", "resident", "dirty", "swap"], match.group(4, 5, 6, 7))
                    ),
                    raw=line,
                )
            )

    topology = capture["topology"]
    ranges = topology["ranges"]
    mapped = []
    totals = Counter()
    for item in ranges:
        lo = item["start"]
        hi = lo + item["capacity"]
        parts = []
        for region in regions:
            begin = max(lo, region["start"])
            end = min(hi, region["end"])
            if begin < end:
                parts.append(
                    dict(start=begin, end=end, bytes=end - begin, type=region["type"])
                )
        assert sum(part["bytes"] for part in parts) == item["capacity"], (
            capture["label"],
            capture["role"],
            item,
            parts,
        )
        totals[item["kind"]] += item["capacity"]
        mapped.append(dict(**item, intersections=parts))

    r07_summary = []
    if capture["role"] == "layout":
        extents = r07_extents(
            {"ranges": ranges}, topology["r07_mapped_bytes"]
        )
        assert topology["r07_mapped_bytes"] == capture["context_before"]["ack"]["state"]["mapped_bytes"]
        r07_summary = [
            {key: extent[key] for key in ("logical_start", "logical_len", "start", "allocation_generation")}
            for extent in extents
        ]
        if capture["label"] == "inflight":
            assert_pinned_identity(
                capture["context_before"]["topology"]["r07_extents"],
                capture["context_after"]["topology"]["r07_extents"],
            )

    before = capture["context_before"]
    after = capture["context_after"]
    assert before["admitted"] == after["admitted"]
    assert before["durable"] == after["durable"]
    assert capture["identity_unchanged"] and capture["stable_phase_epoch_list_endpoints"]
    reports.append(
        dict(
            label=capture["label"],
            role=capture["role"],
            pid=capture["pid"],
            page_size=page,
            tool_begin=capture["tool_begin_monotonic_ns"],
            tool_end=capture["tool_end_monotonic_ns"],
            identity_unchanged=True,
            phase=capture["label"],
            r07_epoch=before["ack"]["state"]["epoch"],
            backing_list_epoch=topology["header"]["epoch"],
            phase_counters=[before["admitted"], before["durable"]],
            logical_r07_extents=r07_summary,
            r07_mapped_bytes=topology["r07_mapped_bytes"],
            range_totals=dict(totals),
            ranges=mapped,
            topology_sha256=topology["sha256"],
            raw_vm_sha256=hashlib.sha256((R / capture["stdout"]).read_bytes()).hexdigest(),
            raw_footprint_before=capture["identity_before"]["last"]["phys_footprint_bytes"],
            raw_footprint_after=capture["identity_after"]["last"]["phys_footprint_bytes"],
            named_region_summary_lines=[
                line
                for line in text.splitlines()
                if line.startswith(
                    (
                        "page table in kernel",
                        "MALLOC metadata",
                        "MALLOC_SMALL",
                        "MALLOC_LARGE",
                        "Stack ",
                        "Memory Tag 22",
                        "DefaultMallocZone",
                    )
                )
            ],
            physical_attribution="exact byte address coverage only; mapping resident/dirty counts may span several owner ranges and are not additive physical credits",
        )
    )

for fact in terminal["facts"]:
    assert fact["kind"] == "actual_repaid_extent_topology_validated"
    assert_repaid_identity(
        fact["before_extents"],
        fact["after_extents"],
        fact["logical_mapped_bytes"],
    )

summary = [
    dict(
        label=report["label"],
        role=report["role"],
        ranges=len(report["ranges"]),
        range_totals=report["range_totals"],
        logical_r07_extents=report["logical_r07_extents"],
        r07_mapped_bytes=report["r07_mapped_bytes"],
        types=dict(
            Counter(
                part["type"]
                for item in report["ranges"]
                for part in item["intersections"]
            )
        ),
        before=report["raw_footprint_before"],
        after=report["raw_footprint_after"],
        page_table_lines=[
            line
            for line in report["named_region_summary_lines"]
            if line.startswith("page table in kernel")
        ],
    )
    for report in reports
]
result = dict(
    source_sha256=terminal["source_sha256"],
    captures=reports,
    summary=summary,
    valid_idle_and_inflight=True,
    cold_generations=cold["generations"],
    continuity="monotonic admitted/durable counters unchanged across each tool interval; external markers gate only admission, scans continue; no atomic group footprint claim",
    physical_credit_authorized=0,
    limits=[
        "VM tools perturb targets and are outside native group; not formal performance evidence",
        "Page table/summary page counts may be rounded; no exact byte credit from multiplication",
        "Address ownership, resident/dirty pages, and whole footprint are distinct",
        "Pool startup list is valid here because the actual L1 pool terminal report has zero grants; not an L2 dynamic list claim",
        "Memory Tag22 and any remaining unowned regions remain unknown",
    ],
)
(D / "address-audit.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(summary, indent=2))
