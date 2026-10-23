"""Read-only independent literal/account/proof checks for the pinned joint fixture.

Usage: python3 check_joint_checkpoint.py FULL_RETAINED_WORLD
This checks the latest complete checkpoint, not WAL replay or general economic balance.
"""
import hashlib
import json
import sys
from pathlib import Path

world = Path(sys.argv[1])
checkpoints = []
for path in world.glob("checkpoint-*.json"):
    value = json.loads(path.read_bytes())
    if "engine" in value:
        checkpoints.append((value["sequence"], path, value))
sequence, path, checkpoint = max(checkpoints, key=lambda row: row[0])
engine = json.loads(bytes(checkpoint["engine"]))
state = engine["world"]
cash = {row["value"]["id"]: row["value"]["cash"]
        for row in state["accounts"]["slots"] if row["value"] is not None}
rent = [row for row in engine["timeline"]["claims"] if row["id"] == 9001]
sources = [row for row in engine["timeline"]["claim_sources"] if row["claim_id"] == 9001]
unit = state["housing"]["state"]["units"][0]
checks = {
    "accounts": cash == {1: 990130, 2: 0, 3: 809870, 4: 1200000},
    "total_cash": sum(cash.values()) == 3000000,
    "rent_single_claim": len(rent) == 1 and all(rent[0][k] == v for k, v in
        {"payer": 2, "payee": 3, "due": 31*86400, "principal": 263226,
         "paid": 189580, "processed": True}.items()),
    "rent_source": len(sources) == 1 and sources[0]["source"] == {
        "kind": "rent", "details": {"landlord": 3, "tenant": 2, "lease_id": 9001, "unit_id": 101}},
    "occupancy": unit["unit_id"] == 101 and unit["household_id"] == 1
        and unit["lease_id"] == 9001 and unit["arrears_ref"] == 9001,
    "one_consumed_offer": state["housing"]["state"]["consumed_offer_ids"] == [9001],
    "market_day31": state["town"]["state"]["settled_day"] == 31,
    "literal_invoice_and_rent": 200000-10000+1600-2*(1000+10) == 189580
        and 263226-189580 == 73646,
}
proofs = {}
for proof_path in world.glob("town-proof-*.json"):
    raw = proof_path.read_bytes()
    proof = json.loads(raw)
    assert proof_path.stem == "town-proof-" + hashlib.sha256(raw).hexdigest()
    if proof["service_day"] in (1, 2, 3):
        day = proof["service_day"]
        assert day not in proofs
        profile_hash = bytes(proof["allocation_profile_sha256"]).hex()
        profile_bytes = (world / ("town-allocation-"+profile_hash+".json")).read_bytes()
        assert hashlib.sha256(profile_bytes).hexdigest() == profile_hash
        proofs[day] = {"profile_hash": profile_hash,
                       "served": proof["assignment"]["served_people"],
                       "outbound": next(row["served_people"] for row in proof["assignment"]["demand_outcomes"] if row["source_id"] == 101),
                       "return": next(row["served_people"] for row in proof["assignment"]["demand_outcomes"] if row["source_id"] == 102),
                       "fee_rate": proof["fee_policy"]["cents_per_travel_second"]}
checks["historical_profiles"] = len(proofs) == 3 and len({p["profile_hash"] for p in proofs.values()}) == 3
checks["transport_zero_fee"] = all(p["fee_rate"] == 0 for p in proofs.values())
checks["access_changes_actual_arrival"] = [proofs[d]["outbound"] for d in (1, 2, 3)] == [4, 0, 4]
checks["directed_return_remains_open"] = [proofs[d]["return"] for d in (1, 2, 3)] == [4, 4, 4]
checks["total_served"] = [proofs[d]["served"] for d in (1, 2, 3)] == [8, 4, 8]
assert all(checks.values()), checks
print(json.dumps({"passed": True, "scope": "fixed_one_unit_joint_checkpoint_and_raw_proofs",
                  "world": str(world), "checkpoint": path.name, "sequence": sequence,
                  "checkpoint_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                  "checks": checks, "accounts": cash, "proofs": proofs}, indent=2))
