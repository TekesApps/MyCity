"""Verify actual-run provenance, fixed-profile arithmetic and observed duration.

Run after copying report.json, process.txt, heartbeat.jsonl and rss-samples.jsonl
into this directory. Uses only Python's standard library; never advances a world.
"""
import hashlib
import json
from pathlib import Path
import re
import struct

here = Path(__file__).resolve().parent
repo = here.parents[2]
report = json.loads((here / "report.json").read_text())
process = (here / "process.txt").read_text()
heartbeats = [json.loads(line) for line in (here / "heartbeat.jsonl").read_text().splitlines()]
rss = [json.loads(line) for line in (here / "rss-samples.jsonl").read_text().splitlines()]
files = [".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
files += [str(p.relative_to(repo)) for d in ("crates", "fixtures") for p in (repo / d).rglob("*") if p.is_file()]
h = hashlib.sha256()
for name in sorted(files):
    content = (repo / name).read_bytes()
    encoded = name.encode()
    h.update(struct.pack("<Q", len(encoded)))
    h.update(encoded)
    h.update(struct.pack("<Q", len(content)))
    h.update(content)
assert h.hexdigest() == report["source_sha256"], "run does not describe current model sources"
assert hashlib.sha256((repo / "Cargo.lock").read_bytes()).hexdigest() == report["lock_sha256"]
assert report["schema"] == "mycity-m0-continuity-v1"
assert report["profile"] == "release"
assert 1800 <= report["requested_wall_seconds"] <= 3600
assert report["workload_wall_seconds"] >= report["requested_wall_seconds"]
assert report["actual_wall_seconds"] >= report["workload_wall_seconds"]
assert report["wall_duration_gate"] is True
assert report["sixteen_gb_acceptance"] is False
assert report["actual_process_exits"] >= 11 and report["all_crash_points_exercised"]
assert report["restart_count"] > report["actual_process_exits"]
assert report["committed_sequence"] > 2048
assert report["cycles"] >= 550
assert report["actual_process_exits"] == report["cycles"] // 50
assert report["restart_count"] == 3 + report["cycles"] // 25 + report["cycles"] // 50
assert report["committed_sequence"] == 46 + 8 * report["cycles"] + report["actual_process_exits"]
assert report["final"]["interaction_time"] == 120 + 30 * report["cycles"]
assert len(heartbeats) >= 50 and len(rss) >= 30
assert "Too many open files" not in process and "RecoveryRequired" not in process
opening = json.loads((repo / "fixtures/m0-exchange-opening.json").read_text())
opening_cash = sum(a["cash"] for a in opening["accounts"])
buyer_cash = next(a["cash"] for a in opening["accounts"] if a["id"] == 2)
contracts = [q for q in opening["exchange"]["quotes"] if q["id"] in (1, 2, 3, 4)]
price = sum(q["unit_price"] * q["quantity"] // 1000 for q in contracts)
freight = sum(10 * sum(edge is not None for edge in q["edges"]) for q in contracts)
obligation = opening["region"]["obligations"][0]
periods = (obligation["expires_at"] - obligation["next_due"]) // obligation["period_seconds"] + 1
handling = 5 * len(contracts)
incidents = 25 * len(contracts)
available_for_external_claim = buyer_cash - price - freight + periods * obligation["amount"] + incidents - handling
expected_unpaid = 5 * 700000 - available_for_external_claim
assert expected_unpaid == 3305000
assert report["initial_full_and_blocked"]["source"]["pending"][0]["event"] == 5
assert report["initial_full_and_blocked"]["source"]["reconciled"] is False
samples = [row["sample"] for row in heartbeats] + [report["after_physical_delivery_and_handoff"], report["final"]]
for sample in samples:
    assert sample["cash_cents"] == opening_cash == 3000000
    assert sample["outstanding_cents"] == expected_unpaid
    assert sample["source"]["pending"] == []
    assert sample["source"]["next_event"] == 6 and sample["source"]["reconciled"] is True
    assert sample["checkpoint"]["wal_bytes"] <= 64 * 1024 * 1024
    assert sample["checkpoint"]["archive_bytes"] <= sample["checkpoint"]["archive_limit"]
assert all(a["cycles"] < b["cycles"] and a["wall_elapsed_seconds"] < b["wall_elapsed_seconds"] for a, b in zip(heartbeats, heartbeats[1:]))
assert all(a["sample"]["checkpoint"]["sequence"] < b["sample"]["checkpoint"]["sequence"] for a, b in zip(heartbeats, heartbeats[1:]))
world = json.loads((here / "world.json").read_text())
inspection = json.loads((here / "inspection.json").read_text())
assert inspection["store_state_sha256"] == report["final"]["state_sha256"]
assert inspection["sequence"] == report["committed_sequence"] == inspection["receipts_scanned"]
assert inspection["unchanged_by_queries"] is True
assert inspection["source_after_read_only_reopen"]["reconciled"] is False
assert inspection["source_after_read_only_reopen"]["pending"] == []
accounts = {a["id"]: a for a in world["accounts"]}
assert [accounts[i]["cash"] for i in (1, 2, 3, 4)] == [1000000, 0, 596800, 1403200]
assert all(a["protected"] == 0 for a in accounts.values())
stocks = {s["id"]: s for s in world["stocks"]}
assert stocks[7]["quantity"] == 1000
assert all(stocks[i]["quantity"] == 1000 and stocks[i]["basis"] == 2020 for i in (8, 9, 10, 11))
assert stocks[2]["quantity"] == 0 and stocks[6]["quantity"] == 100000
assert stocks[4]["quantity"] + stocks[5]["quantity"] == 6000000
assert stocks[4]["quantity"] == 5400000 and stocks[5]["quantity"] == 600000
capacity = json.loads((here / "capacity-rows.json").read_text())
resources = [row["value"] for row in capacity if row["type"] == "resource"]
assert len(resources) == 6
assert all(r["reserved"] == 0 and r["occupied"] == 0 for r in resources)
assert not any(row["type"] == "lease" for row in capacity)
exchange = world["exchange"]
assert all(ship is None for ship in exchange["shipments"])
assert sum(q["status"] == "Delivered" for q in exchange["quotes"]) == 4
assert exchange["inbox"]["accrued"] == 3500120
assert exchange["inbox"]["paid"] == 195120
assert all(f is None for f in exchange["inbox"]["facts"])
assert sum(f is not None for f in exchange["inbox"]["retained"]) == 5
cities = [c for c in world["region"]["cities"] if c is not None]
assert sum(c["population"] for c in cities) == sum(c["population"] for c in opening["region"]["cities"])
assert sum(c["development_elapsed"] for c in cities) == sum(c["development_elapsed"] for c in opening["region"]["cities"]) + report["final"]["interaction_time"]
base_cities = {c["id"]: c for c in opening["region"]["cities"]}
for city in cities:
    active_cycles = (report["cycles"] + 2 - (city["id"] - 1)) // 3
    expected_age = base_cities[city["id"]]["development_elapsed"] + 30 * active_cycles + (120 if city["id"] == 2 else 0)
    assert city["development_elapsed"] == expected_age
traces = json.loads((here / "trace-receipts.json").read_text())
created_facts = {}
settlements = {}
for row in traces:
    record = row["record"]
    outcome = record["outcome"].get("Ok")
    if outcome is None:
        continue
    for i, action in enumerate(record["actions"]):
        if action is None:
            continue
        key = outcome["created_facts"][i]
        if key is not None:
            assert key not in created_facts
            created_facts[key] = action
        paid = outcome["settled"][i]
        if paid is not None:
            key = action["command"]["sequence"]
            settlements[key] = settlements.get(key, 0) + paid
assert set(created_facts) == set(range(1, 14))
assert all(settlements[i] == 5 for i in range(1, 5))
assert all(settlements[i] == 25 for i in range(5, 9))
assert settlements[9] == 195000
assert sum(settlements.values()) == 195120
peak = re.search(r"(\d+)\s+maximum resident set size", process)
assert peak, "missing actual process memory observation"
owned = [s["owned_capacity_bytes"] for s in samples]
loop_owned = [row["sample"]["owned_capacity_bytes"] for row in heartbeats]
# The Rust maximum is sampled inside the workload loop, not during warmup/final reopen.
# A no-tail checkpoint restore gives the six Resource rows Vec capacity eight;
# prepare/replay clones to length six. Four u32 per Resource => 32 fixed extra bytes.
assert max(loop_owned) <= report["max_owned_capacity_bytes"]
assert len(set(loop_owned)) == 1
assert max(owned) <= report["max_owned_capacity_bytes"] + (8 - len(resources)) * 16
summary = {
    "source_sha256": report["source_sha256"],
    "workload_wall_seconds": report["workload_wall_seconds"],
    "total_wall_seconds": report["actual_wall_seconds"],
    "cycles": report["cycles"],
    "committed_sequence": report["committed_sequence"],
    "actual_process_exits": report["actual_process_exits"],
    "restart_count": report["restart_count"],
    "cash_cents": opening_cash,
    "external_original_claim_cents": 3500000,
    "actual_external_payment_cents": available_for_external_claim,
    "unpaid_claim_cents": expected_unpaid,
    "owned_bytes_min": min(owned),
    "owned_bytes_max": max(owned),
    "loop_owned_bytes_max_reported": report["max_owned_capacity_bytes"],
    "checkpoint_restore_resource_capacity_delta_bytes": max(owned)-min(loop_owned),
    "time_reported_peak_rss_bytes": int(peak.group(1)),
    "sampled_parent_rss_bytes_max": max(s["parent_rss_kib"] for s in rss) * 1024,
    "sampled_tree_rss_bytes_max": max(s["sampled_tree_rss_kib"] for s in rss) * 1024,
    "rss_samples": len(rss),
    "heartbeat_samples": len(heartbeats),
    "archive_bytes_final": report["final"]["checkpoint"]["archive_bytes"],
    "max_cycle_us": report["max_cycle_us"],
    "max_invariant_check_us": report["max_invariant_check_us"],
    "sixteen_gb_acceptance": False,
    "scope": report["scope"],
    "result": "passed_finite_headless_continuity_scope",
}
print(json.dumps(summary, indent=2, ensure_ascii=False))
