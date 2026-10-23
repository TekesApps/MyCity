#!/usr/bin/env python3
"""Independent literal oracle for the retained household-wage/spatial checkpoints.

This reads checkpoint/genesis/proof JSON and prior receipts only. It does not open a
Store, execute model commands, mutate evidence, or claim a performance measurement.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

PREFIX = "HOUSEHOLD_WAGE_SPATIAL_RESULT="
VERIFY_PREFIX = "HOUSEHOLD_WAGE_SPATIAL_VERIFY_OPEN="
ROOT = Path("/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01")
EXPECTED = {
    "control": {
        "cash": {1: 900051, 2: 590, 3: 699359, 4: 1200000, 5: 5000},
        "sales": [7, 3],
        "day1_household_cash": 93,
        "tax_basis": [63, 0],
        "tax_assessed": [31, 0],
        "retained": -741,
        "household_dividend": 0,
        "government_dividend": 0,
        "dividend_total": 0,
        "sequence": 79,
        "state_hash": "cbff86fc5bfdab2b1263b49492552b09ce53d980facbb1557d59df420d214ea5",
        "raw_remaining": 20,
    },
    "higher-wage": {
        "cash": {1: 900130, 2: 137, 3: 700200, 4: 1200013, 5: 4520},
        "sales": [10, 10],
        "day1_household_cash": 30,
        "tax_basis": [90, 90],
        "tax_assessed": [45, 45],
        "retained": 90,
        "household_dividend": 77,
        "government_dividend": 13,
        "dividend_total": 90,
        "sequence": 81,
        "state_hash": "68bdebd9decd7d3af1e3bf149b53440ad0c88e0bc501aeca5f6cac8085e4e030",
        "raw_remaining": 10,
    },
}


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(",", ":")).encode("utf-8")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prefixed_json(path: Path, prefix: str) -> dict[str, Any]:
    matches = [line[len(prefix):] for line in path.read_text(encoding="utf-8").splitlines()
               if line.startswith(prefix)]
    assert len(matches) == 1, f"{path}: expected exactly one {prefix} record, got {len(matches)}"
    value = json.loads(matches[0])
    assert isinstance(value, dict)
    return value


def latest_checkpoint(world: Path) -> tuple[Path, dict[str, Any]]:
    files = []
    for path in world.glob("checkpoint-[0-9]*.json"):
        if re.fullmatch(r"checkpoint-[0-9]+\.json", path.name):
            value = json.loads(path.read_bytes())
            if "engine" in value:
                files.append((int(value["sequence"]), path, value))
    assert files, f"no full checkpoint in {world}"
    _, path, checkpoint = max(files, key=lambda row: row[0])
    return path, checkpoint


def engine_bytes(checkpoint: dict[str, Any]) -> dict[str, Any]:
    # Checkpoint stores the exact serialized Engine bytes as a JSON integer array.
    return json.loads(bytes(checkpoint["engine"]))


def view_hashes(engine: dict[str, Any], genesis: dict[str, Any]) -> tuple[str, str]:
    """Recreate Engine::state_hash_with and Store::state_hash from checkpoint fields."""
    world = engine["world"]
    accounts_by_id = {
        slot["value"]["id"]: slot["value"]
        for slot in world["accounts"]["slots"] if slot["value"] is not None
    }
    account_id_by_handle_index = {
        handle["raw"]["index"]: account_id
        for account_id, handle in world["account_ids"]
    }
    stocks_by_id = {
        slot["value"]["id"]: slot["value"]
        for slot in world["stocks"]["slots"] if slot["value"] is not None
    }
    account_views = []
    for account_id, _handle in world["account_ids"]:
        account = accounts_by_id[account_id]
        account_views.append({key: account[key] for key in
                              ("id", "role", "cash", "protected", "opening_source_id")
                              if key in account})
    stock_views = []
    for stock_id, _handle in world["stock_ids"]:
        stock = stocks_by_id[stock_id]
        owner_index = stock["owner"]["raw"]["index"]
        stock_views.append({
            "id": stock["id"],
            "owner": account_id_by_handle_index[owner_index],
            **{key: stock[key] for key in (
                "commodity", "quantity", "basis", "lot_id", "origin_quantity",
                "origin_basis", "offset", "opening_source_id", "opening_basis")
               if key in stock},
        })
    world_view: dict[str, Any] = {
        "schema": "mycity-m0-state-v7",
        "scope": world["scope"],
        "seed": world["seed"],
        "revision": world["revision"],
        "accounts": account_views,
        "stocks": stock_views,
        "opening_sources": world["sources"],
        "region": world["region"],
        "economy": world["economy"],
        "boot": world["boot"],
        "exchange": world["exchange"],
    }
    for optional in ("construction", "town"):
        if world.get(optional) is not None:
            world_view[optional] = world[optional]
    assert "housing" not in engine and "housing" not in world_view

    engine_view: dict[str, Any] = {
        "world": world_view,
        "resources": engine["resources"],
        "leases": engine["leases"],
        "next_token": engine["next_token"],
        "timeline": engine["timeline"],
        "background": engine["background"],
    }
    if engine.get("spatial_permissions") is not None:
        engine_view["spatial_permissions"] = engine["spatial_permissions"]
    engine_hash = hashlib.sha256(canonical(engine_view)).hexdigest()
    store_hash = hashlib.sha256(canonical({
        "schema": "mycity-m0-store-state-v1",
        "engine": engine_hash,
        "spatial": genesis["spatial"],
    })).hexdigest()
    return engine_hash, store_hash


def validate_market_proofs(world: Path, state: dict[str, Any]) -> list[str]:
    allocation_files = list(world.glob("town-allocation-*.json"))
    assert len(allocation_files) == 1, f"{world}: expected one immutable allocation profile"
    allocation_path = allocation_files[0]
    allocation_sha = sha256(allocation_path)
    assert allocation_path.stem == f"town-allocation-{allocation_sha}"

    proofs = []
    for path in world.glob("town-proof-*.json"):
        digest = sha256(path)
        assert path.stem == f"town-proof-{digest}", f"proof filename/hash mismatch: {path}"
        proof = json.loads(path.read_bytes())
        assert bytes(proof["allocation_profile_sha256"]).hex() == allocation_sha
        proofs.append((proof["service_day"], path, proof))
    proofs.sort(key=lambda row: row[0])
    assert [day for day, _, _ in proofs] == [1, 2, 3], f"unexpected service days: {proofs}"
    expected_od = [(101, 4, 4, 0, 81), (102, 4, 4, 0, 81)]
    proof_hashes = []
    for day, path, proof in proofs:
        outcomes = proof["assignment"]["demand_outcomes"]
        od = [(row["source_id"], row["requested_people"], row["served_people"],
               row["unserved_people"], row["mean_travel_seconds"]) for row in outcomes]
        assert od == expected_od, f"{path}: unexpected OD assignment {od}"
        assert proof["assignment"]["served_people"] == 8
        assert proof["assignment"]["unserved_people"] == 0
        flows = proof["assignment"]["edge_flows"]
        assert [(row["edge_id"], row["people"], row["capacity_people"])
                for row in flows] == [(21, 4, 4), (22, 4, 4), (23, 4, 4), (24, 4, 4)]
        assert proof["partitions"] == [{
            "quote_id": 1, "outbound_source": 101, "return_source": 102,
            "trip_count": 4, "goods_per_trip": 10,
        }]
        proof_hashes.append(sha256(path))
    last = state["last_transport"]
    assert bytes(last["issued_proof_sha256"]).hex() == proof_hashes[-1]
    assert last["issued_day"] == 3 and last["consumed_generation"] == 3
    return proof_hashes


def validate_cold_reader(variant: str, expected: dict[str, Any]) -> dict[str, Any]:
    label = "control" if variant == "control" else "higher"
    receipt_dir = ROOT / f"{441 if variant == 'control' else 442}-wage-{label}-cold"
    reader_dir = ROOT / f"{441 if variant == 'control' else 442}-wage-{label}-cold-reader"
    receipt = json.loads((receipt_dir / "receipt.json").read_bytes())
    assert receipt["full_copy_verified"] is True and receipt["original_unchanged"] is True
    assert receipt["reader"]["exit_code"] == 0 and receipt["reader"]["source_unchanged"] is True
    reader = prefixed_json(reader_dir / "stdout.log", VERIFY_PREFIX)
    assert reader["mode"] == "verify_open_working_copy"
    assert reader["model_commands_executed"] == 0
    assert reader["receipt_sequence"] == expected["sequence"]
    assert reader["state_hash"] == expected["state_hash"]
    assert reader["native_peak"] == "not_measured"
    return {"receipt": receipt, "reader": reader}


def validate_variant(name: str, report: dict[str, Any], worlds: Path) -> dict[str, Any]:
    expected = EXPECTED[name]
    section = report["higher_wage" if name == "higher-wage" else name]
    world = worlds / name
    assert world.is_dir()
    checkpoint_path, checkpoint = latest_checkpoint(world)
    engine = engine_bytes(checkpoint)
    genesis = json.loads((world / "genesis.json").read_bytes())
    assert genesis["spatial"] is not None, f"{name}: spatial genesis reference missing"
    assert checkpoint["sequence"] == expected["sequence"]
    assert section["final_cold_receipt_sequence"] == expected["sequence"]
    assert section["final_cold_state_hash"] == expected["state_hash"]
    assert section["state_hash"] == expected["state_hash"]
    engine_hash, state_hash = view_hashes(engine, genesis)
    assert state_hash == expected["state_hash"], (name, state_hash, expected["state_hash"])

    w = engine["world"]
    t = w["town"]["state"]
    accounts = {slot["value"]["id"]: slot["value"]
                for slot in w["accounts"]["slots"] if slot["value"] is not None}
    cash = {account_id: row["cash"] for account_id, row in accounts.items()}
    assert cash == expected["cash"], (name, cash)
    assert sum(cash.values()) == 2_805_000
    assert section["sales_by_day"] == expected["sales"]
    assert section["household_cash_after_day1"] == expected["day1_household_cash"]
    assert section["household_cash_final"] == expected["cash"][2]
    assert section["taxable_basis_by_day"] == expected["tax_basis"]
    assert section["tax_assessed_by_day"] == expected["tax_assessed"]
    assert section["retained_earnings_before_distribution"] == expected["retained"]
    assert section["household_dividend"] == expected["household_dividend"]
    assert section["government_dividend"] == expected["government_dividend"]
    assert section["dividend_total"] == expected["dividend_total"]

    stocks = {slot["value"]["id"]: slot["value"]
              for slot in w["stocks"]["slots"] if slot["value"] is not None}
    assert stocks[2]["quantity"] == expected["raw_remaining"]
    assert stocks[3]["quantity"] == 0
    assert sum(row["paid"] for row in t["payroll"]) == (
        section["seller_payroll_paid"] + section["supplemental_payroll_paid"])

    claims = engine["timeline"]["claims"]
    sources = engine["timeline"].get("claim_sources", [])
    assert all(claim["paid"] == claim["principal"] and claim["processed"] for claim in claims)
    assert [source["source"]["details"]["taxable_basis"] for source in sources] == expected["tax_basis"][:len(sources)]
    assert [source["source"]["details"]["assessed"] for source in sources] == expected["tax_assessed"][:len(sources)]
    assert all(source["source"]["kind"] == "tax" for source in sources)

    registers = t["equity_registers"]
    assert len(registers) == 1
    register = registers[0]
    holders = {holder["account"]: holder for holder in register["holders"]}
    assert register["issued_shares"] == 7003
    assert holders[2]["shares"] == 6003 and holders[1]["shares"] == 1000
    assert register.get("cumulative_dividends_distributed", 0) == expected["dividend_total"]
    cursors = t["tax_cursors"]
    assert len(cursors) == 1
    assert cursors[0]["cumulative_assessed"] == sum(expected["tax_assessed"])
    assert cursors[0]["retained_earnings_cents"] == (
        0 if name == "higher-wage" else expected["retained"])

    proof_hashes = validate_market_proofs(world, t)
    cold = validate_cold_reader(name, expected)
    assert cold["reader"]["engine_owned_bytes"] == section["final_cold_engine_owned_bytes"]
    assert cold["reader"]["world_owned_bytes"] == section["final_cold_world_owned_bytes"]
    assert cold["reader"]["store_owned_bytes"] > 0
    return {
        "checkpoint": checkpoint_path.name,
        "checkpoint_sha256": sha256(checkpoint_path),
        "sequence": checkpoint["sequence"],
        "engine_hash": engine_hash,
        "state_hash": state_hash,
        "cash": cash,
        "total_cash": sum(cash.values()),
        "proof_sha256_by_day": proof_hashes,
        "cold_reader_mode_commands": cold["reader"]["model_commands_executed"],
        "owned_bytes": {
            "cli_final_cold_store": section["final_cold_store_owned_bytes"],
            "cli_final_cold_engine": section["final_cold_engine_owned_bytes"],
            "cli_final_cold_world": section["final_cold_world_owned_bytes"],
            "independent_reader_store": cold["reader"]["store_owned_bytes"],
        },
    }


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} CLI_STDOUT WORLDS_ROOT")
    cli_stdout, worlds = Path(sys.argv[1]), Path(sys.argv[2])
    report = prefixed_json(cli_stdout, PREFIX)
    assert report["total_cash"] == 2_805_000
    assert report["native_peak"] == "not_measured"
    assert "fixed OD" in report["route"]
    build = json.loads((ROOT / "439-wage-final-cli-build/exit.json").read_bytes())
    run = json.loads((ROOT / "440-wage-spatial-cli/exit.json").read_bytes())
    assert build["exit_code"] == run["exit_code"] == 0
    assert build["source_unchanged"] is True and run["source_unchanged"] is True
    results = {name: validate_variant(name, report, worlds) for name in EXPECTED}
    assert results["control"]["proof_sha256_by_day"] == results["higher-wage"]["proof_sha256_by_day"]
    assert report["control"]["served_people_by_day"] == report["higher_wage"]["served_people_by_day"]
    assert report["control"]["served_people_by_day"] == [[4, 4], [4, 4], [4, 4]]
    print(json.dumps({
        "passed": True,
        "scope": "independent literal checkpoint/hash/cash/equity/tax/transport oracle",
        "model_commands_executed_by_oracle": 0,
        "build_receipt": build,
        "cli_receipt": run,
        "results": results,
        "limitations": [
            "The oracle verifies retained checkpoints and independent cold-open receipts; it does not replay model commands.",
            "OD/trip sources are fixed and identical; this does not prove wage or migration changes traffic.",
            "The measured owned-capacity counters are not process peak memory or a 16GB acceptance.",
        ],
    }, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
