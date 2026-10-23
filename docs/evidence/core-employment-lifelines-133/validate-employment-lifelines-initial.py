#!/usr/bin/env python3
"""Independent post-run checks for the seven town_lifelines modes.

This reads only retained receipts, logs, genesis/checkpoint data, and the fixed
opening fixture. It never launches Rust, Cargo, or a model. Missing run evidence
is reported as incomplete, not treated as zero or as success.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shlex
import sys
from pathlib import Path
from typing import Any


DEFAULT_WORK = Path("/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01")
DEFAULT_RUN = DEFAULT_WORK / "117-employment-lifelines-cases"
DEFAULT_HISTORY = DEFAULT_WORK / "108-canonical-integrated-clis"
EXPECTED_CASES = {
    "normal",
    "held",
    "generator-failure",
    "construction-utilities",
    "construction-held",
    "employment-control",
    "employment-change",
}


class EvidenceError(Exception):
    pass


def require(condition: bool, label: str, detail: str = "") -> None:
    if not condition:
        raise EvidenceError(f"{label}{': ' + detail if detail else ''}")


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise EvidenceError(f"missing:{path}") from exc
    except (OSError, json.JSONDecodeError) as exc:
        raise EvidenceError(f"invalid_json:{path}:{exc}") from exc


def parse_kv_line(line: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for token in shlex.split(line.strip()):
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        values[key] = value
    return values


def parse_stdout(path: Path) -> tuple[dict[str, str], dict[str, str]]:
    try:
        lines = path.read_text().splitlines()
    except FileNotFoundError as exc:
        raise EvidenceError(f"missing_stdout:{path}") from exc
    for line in lines:
        if line.startswith("mode="):
            mode = parse_kv_line(line)
            scope = next((parse_kv_line(item) for item in lines if item.startswith("scope=")), {})
            require(bool(scope), "missing_scope_receipt", str(path))
            return mode, scope
    raise EvidenceError(f"no_success_mode_line:{path}")


def int_field(row: dict[str, str], name: str) -> int:
    if name not in row:
        raise EvidenceError(f"missing_field:{name}")
    try:
        return int(row[name])
    except ValueError as exc:
        raise EvidenceError(f"not_integer:{name}={row[name]}") from exc


def bool_field(row: dict[str, str], name: str) -> bool:
    value = row.get(name)
    if value not in {"true", "false"}:
        raise EvidenceError(f"missing_or_invalid_bool:{name}={value}")
    return value == "true"


def stdout_path(case_dir: Path) -> Path:
    for name in ("stdout.log", "stdout.txt", "transcript.log"):
        candidate = case_dir / name
        if candidate.is_file():
            return candidate
    raise EvidenceError(f"case_stdout_not_found:{case_dir}")


def canonical_case(record: dict[str, Any], case_dir: Path) -> str:
    name = str(record.get("case", case_dir.name)).lower().replace("_", "-")
    aliases = {
        "control": "employment-control",
        "employment-control": "employment-control",
        "change": "employment-change",
        "employment-change": "employment-change",
        "construction-utilities-held": "construction-held",
    }
    if name in aliases:
        return aliases[name]
    try:
        mode, _ = parse_stdout(stdout_path(case_dir))
    except EvidenceError:
        return name
    if mode.get("employment") == "employment-control":
        return "employment-control"
    if mode.get("employment") == "employment-change":
        return "employment-change"
    if mode.get("mode") == "normal":
        return "normal"
    if mode.get("mode") == "hold-road-inspection":
        return "held"
    if mode.get("mode") == "generator-failure":
        return "generator-failure"
    if mode.get("mode") == "construction-utilities-three-day":
        return "construction-utilities"
    if mode.get("mode") == "construction-utilities-three-day-held":
        return "construction-held"
    return name


def get_store_dir(record: dict[str, Any], case_dir: Path) -> Path:
    argv = record.get("argv", [])
    if isinstance(argv, list):
        for index, item in enumerate(argv):
            if item == "--store-dir" and index + 1 < len(argv):
                return Path(argv[index + 1])
            if isinstance(item, str) and item.startswith("--store-dir="):
                return Path(item.split("=", 1)[1])
    for candidate in (case_dir / "world", case_dir / "store"):
        if candidate.is_dir():
            return candidate
    raise EvidenceError(f"store_dir_not_found:{case_dir}")


def get_exit_code(record: dict[str, Any], case_dir: Path) -> int:
    if isinstance(record.get("exit_code"), int):
        return record["exit_code"]
    for name in ("exit.json", "terminal.json"):
        status_path = case_dir / name
        if status_path.is_file():
            status = read_json(status_path)
            code = status.get("exit_code")
            if isinstance(code, int):
                return code
    raise EvidenceError(f"missing_process_exit_code:{case_dir}")


def load_case_records(run_root: Path) -> dict[str, tuple[dict[str, Any], Path]]:
    cases_dir = run_root / "cases"
    results_path = cases_dir / "results.json"
    if not results_path.is_file():
        results_path = run_root / "results.json"
        cases_dir = run_root / "cases"
    records = read_json(results_path)
    if not isinstance(records, list):
        raise EvidenceError(f"results_not_array:{results_path}")
    found: dict[str, tuple[dict[str, Any], Path]] = {}
    for record in records:
        if not isinstance(record, dict):
            continue
        case_dir = cases_dir / str(record.get("case", ""))
        if not case_dir.is_dir():
            case_dir = cases_dir / str(record.get("case", "")).replace("_", "-")
        if not case_dir.is_dir():
            raise EvidenceError(f"case_directory_missing:{record.get('case')}:{case_dir}")
        key = canonical_case(record, case_dir)
        if key in found:
            raise EvidenceError(f"duplicate_case:{key}")
        found[key] = (record, case_dir)
    return found


def checked_case(
    cases: dict[str, tuple[dict[str, Any], Path]], key: str
) -> tuple[dict[str, str], dict[str, str], Path, dict[str, Any]]:
    if key not in cases:
        raise EvidenceError(f"missing_case:{key}")
    record, case_dir = cases[key]
    code = get_exit_code(record, case_dir)
    require(code == 0, "case_failed", f"{key}:exit={code}")
    mode, scope = parse_stdout(stdout_path(case_dir))
    store_dir = get_store_dir(record, case_dir)
    return mode, scope, store_dir, record


def latest_checkpoint(world: Path) -> Path:
    checkpoints = []
    for path in world.glob("checkpoint-*.json"):
        match = re.fullmatch(r"checkpoint-(\d+)\.json", path.name)
        if match:
            checkpoints.append((int(match.group(1)), path))
    if not checkpoints:
        raise EvidenceError(f"no_checkpoint:{world}")
    return max(checkpoints)[1]


def decode_store(world: Path) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    genesis = read_json(world / "genesis.json")
    try:
        opening_bytes = bytes(genesis["opening"])
        opening = json.loads(opening_bytes)
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        raise EvidenceError(f"bad_genesis_opening:{world}:{exc}") from exc
    checkpoint_path = latest_checkpoint(world)
    checkpoint = read_json(checkpoint_path)
    try:
        engine = json.loads(bytes(checkpoint["engine"]))
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        raise EvidenceError(f"bad_checkpoint_engine:{checkpoint_path}:{exc}") from exc
    return genesis, opening, engine


def indexed_rows(container: dict[str, Any]) -> dict[int, dict[str, Any]]:
    slots = container.get("slots")
    if not isinstance(slots, list):
        raise EvidenceError("slot_table_missing")
    rows: dict[int, dict[str, Any]] = {}
    for slot in slots:
        value = slot.get("value") if isinstance(slot, dict) else None
        if isinstance(value, dict) and isinstance(value.get("id"), int):
            rows[value["id"]] = value
    return rows


def source_capital(opening: dict[str, Any]) -> dict[str, Any]:
    sources = {row["id"]: row for row in opening.get("sources", [])}
    accounts = opening.get("accounts", [])
    require(bool(sources) and bool(accounts), "opening_sources_or_accounts_missing")
    total = 0
    per_account = {}
    for account in accounts:
        account_id, cash, source_id = account["id"], account["cash"], account["source"]
        source = sources.get(source_id)
        require(source is not None, "account_source_missing", str(account_id))
        require(source.get("kind") == "cash_capital", "account_source_not_cash", str(account_id))
        require(source.get("amount") == cash, "account_source_amount_mismatch", str(account_id))
        total += cash
        per_account[account_id] = cash
    stock_basis_total = 0
    for stock in opening.get("stocks", []):
        source = sources.get(stock.get("source"))
        require(source is not None, "stock_source_missing", str(stock.get("id")))
        require(source.get("kind") == "inventory_capital", "stock_source_not_inventory", str(stock.get("id")))
        require(source.get("amount") == stock.get("basis"), "stock_source_basis_mismatch", str(stock.get("id")))
        stock_basis_total += stock["basis"]
    return {
        "total_cash_cents": total,
        "inventory_basis_cents": stock_basis_total,
        "source_backed_capital_basis_cents": total + stock_basis_total,
        "accounts": per_account,
        "sources": sources,
    }


def transport_signature(proof: dict[str, Any]) -> dict[str, Any]:
    assignment = proof.get("assignment", {})
    opening = proof.get("opening", {})
    return {
        "edges": [
            {key: edge.get(key) for key in ("id", "from", "to", "capacity_people", "mode_mask", "open")}
            for edge in opening.get("edges", [])
        ],
        "entrances": [
            {key: row.get(key) for key in ("id", "owner", "node", "mode_mask", "authorized")}
            for row in opening.get("entrances", [])
        ],
        "demands": [
            {key: row.get(key) for key in ("source_id", "origin_entrance", "destination_entrance", "mode", "people")}
            for row in opening.get("demands", [])
        ],
        "flows": [
            {key: row.get(key) for key in ("edge_id", "people", "capacity_people")}
            for row in assignment.get("edge_flows", [])
        ],
        "outcomes": [
            {key: row.get(key) for key in ("source_id", "requested_people", "served_people", "unserved_people")}
            for row in assignment.get("demand_outcomes", [])
        ],
        "paths": [
            {key: row.get(key) for key in ("source_id", "people", "edge_ids")}
            for row in assignment.get("path_receipts", [])
        ],
        "partitions": proof.get("partitions"),
    }


def load_proof(world: Path, digest: str, day: int) -> dict[str, Any]:
    path = world / f"town-proof-{digest}.json"
    raw = path.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == digest, "proof_filename_hash_mismatch", path.name)
    proof = json.loads(raw)
    require(proof.get("service_day") == day, "proof_wrong_service_day", str(path))
    return proof


HISTORICAL_FIELDS = {
    "normal": {
        "day1_produced": "0", "day1_sales": "0", "day2_produced": "100", "day2_sales": "100",
        "road_capacity_after_inspection": "10", "cash_conserved": "true", "cold_open_state_hash_equal": "true",
        "state_hash": "4c9cab6fb9765f78da94dc44f6d135e3bfff28a10d9234a9fd6bd0be6af963eb",
    },
    "held": {
        "day1_produced": "0", "day1_sales": "0", "day2_produced": "100", "day2_sales": "20",
        "road_capacity_after_inspection": "2", "inspection_held": "true", "cash_conserved": "true",
        "state_hash": "0279a794cd9aa51193f0e0535840ea4958c4f1d7c2ed00ed4f2be9d94ba26577",
    },
    "generator-failure": {
        "day1_produced": "0", "day1_sales": "0", "day2_produced": "0", "day2_sales": "0",
        "road_capacity_after_inspection": "10", "boot_electricity_export_total": "0", "cash_conserved": "true",
        "state_hash": "e3d18aee9f8f523f4e542cc4703910ae32118774b91a00795fb830e074672bc9",
    },
    "construction-utilities": {
        "day1_produced": "100", "day2_produced": "0", "day3_produced": "0",
        "day1_sales": "20", "day2_sales": "20", "day3_sales": "60",
        "day1_capacity": "2", "day2_capacity": "2", "day3_capacity": "10",
        "electricity_used": "400000", "water_used": "400000", "electricity_fee": "8000", "water_fee": "4000",
        "cash_conserved": "true", "final_cold_open_equal": "true",
        "state_hash": "06f8736dcfb9fb88d0edd6863e0157ee860fa19a841706547070abfac14d3aa6",
    },
    "construction-held": {
        "day1_produced": "100", "day2_produced": "0", "day3_produced": "0",
        "day1_sales": "20", "day2_sales": "20", "day3_sales": "20",
        "day1_capacity": "2", "day2_capacity": "2", "day3_capacity": "2",
        "inspection_held": "true", "cash_conserved": "true", "final_cold_open_equal": "true",
        "state_hash": "e2f7eb072f2f2825c9ffb04d113fa210f1c7e94c8bacbd909938c002281caa72",
    },
}


def verify_historical_regressions(current: dict[str, tuple[dict[str, Any], Path]], history_root: Path) -> dict[str, Any]:
    history_cases = history_root / "cases"
    results = read_json(history_cases / "results.json")
    historical_by_name = {str(row.get("case")): row for row in results if isinstance(row, dict)}
    out = {}
    for name, expected in HISTORICAL_FIELDS.items():
        require(name in historical_by_name, "historical_case_missing", name)
        old = historical_by_name[name]
        require(old.get("exit_code") == 0, "historical_case_not_successful", name)
        old_mode, _ = parse_stdout(history_cases / name / "stdout.log")
        mode, _, _, _ = checked_case(current, name)
        for field, baseline in expected.items():
            require(old_mode.get(field) == baseline, "historical_baseline_mismatch", f"{name}.{field}")
            require(mode.get(field) == baseline, "current_old_mode_regression", f"{name}.{field}:{mode.get(field)}!={baseline}")
        for field in expected:
            require(mode[field] == old_mode[field], "historical_current_diff", f"{name}.{field}")
        out[name] = {field: mode[field] for field in expected}
    return out


def verify_opening_pair(
    control_world: Path, change_world: Path, fixture_path: Path
) -> dict[str, Any]:
    control_genesis, control_opening, _ = decode_store(control_world)
    change_genesis, change_opening, _ = decode_store(change_world)
    control_genesis_bytes = (control_world / "genesis.json").read_bytes()
    change_genesis_bytes = (change_world / "genesis.json").read_bytes()
    require(control_genesis_bytes == change_genesis_bytes, "employment_control_change_genesis_files_differ")
    cb, xb = bytes(control_genesis["opening"]), bytes(change_genesis["opening"])
    require(cb == xb, "employment_control_change_genesis_opening_bytes_differ")
    ccap = source_capital(control_opening)
    xcap = source_capital(change_opening)
    require(ccap == xcap, "employment_control_change_opening_capital_differs")
    require(ccap["total_cash_cents"] == 3_101_000, "employment_opening_cash_total_unexpected", str(ccap["total_cash_cents"]))
    require(ccap["accounts"].get(5) == 0 and ccap["accounts"].get(8) == 1_000, "source_capital_transfer_not_visible")
    fixture = read_json(fixture_path)
    source_ids = {row["id"] for row in fixture["sources"]}
    require(18 in source_ids and 25 not in source_ids, "fixture_base_does_not_match_expected_transfer")
    opening_sources = ccap["sources"]
    require(opening_sources[18]["amount"] == 0, "household_original_source_not_zeroed")
    require(opening_sources[25]["amount"] == 1_000, "new_employer_capital_source_not_present")
    # Full genesis bytes include opening, spatial, resource and Boot/construction inputs.
    return {
        "full_genesis_files_equal": True,
        "genesis_file_sha256": hashlib.sha256(control_genesis_bytes).hexdigest(),
        "opening_bytes_equal": True,
        "opening_sha256": hashlib.sha256(cb).hexdigest(),
        "source_backed_total_cash_cents": ccap["total_cash_cents"],
        "source_backed_inventory_basis_cents": ccap["inventory_basis_cents"],
        "source_backed_capital_basis_cents": ccap["source_backed_capital_basis_cents"],
        "household_5_open_cash_cents": ccap["accounts"][5],
        "new_employer_8_open_cash_cents": ccap["accounts"][8],
        "source_18_zeroed": opening_sources[18]["amount"] == 0,
        "source_25_added": opening_sources[25]["amount"] == 1_000,
    }


def verify_employment_pair(
    control: tuple[dict[str, str], dict[str, str], Path, dict[str, Any]],
    change: tuple[dict[str, str], dict[str, str], Path, dict[str, Any]],
    fixture_path: Path,
) -> dict[str, Any]:
    c, _, cworld, _ = control
    x, _, xworld, _ = change
    require(c.get("employment") == "employment-control", "wrong_control_mode")
    require(x.get("employment") == "employment-change", "wrong_change_mode")
    for scope in (control[1], change[1]):
        require(scope.get("employment_assignment") == "trusted_fixture_input", "employment_scope_not_explicit")
        require(scope.get("endogenous_job_search") == "not_implemented", "job_search_limit_not_reported")
        require(scope.get("legal_employment_approval") == "not_modeled", "legal_flow_limit_not_reported")
        require(scope.get("continuous_supply") == "not_implemented" and scope.get("balance") == "not_calibrated", "operating_scope_limit_not_reported")
    opening = read_json(fixture_path)
    household = opening["town"]["households"][0]
    shop = opening["town"]["shops"][0]
    delivery = opening["town"]["delivery"][0]
    partition = None
    for p in opening["transport_allocation"]["partitions"] if "transport_allocation" in opening else []:
        partition = p
    # The profile is stored beside each Store; the versioned fixture carries only
    # the demand/quote/recipe inputs, so require the persisted allocation profile.
    allocations = sorted(cworld.glob("town-allocation-*.json"))
    require(bool(allocations), "control_allocation_profile_missing")
    profile_raw = allocations[0].read_bytes()
    profile = json.loads(profile_raw)
    profile_digest = hashlib.sha256(profile_raw).hexdigest()
    require(allocations[0].stem == f"town-allocation-{profile_digest}", "allocation_profile_filename_hash_mismatch")
    parts = profile.get("partitions", [])
    require(len(parts) == 1, "unexpected_transport_partition_count")
    partition = parts[0]
    shelf_price = shop["shelf_price_per_q"]
    base_need = household["need_per_household"] * household["households"]
    require(shelf_price == 1 and base_need == 100, "fixture_need_or_shelf_price_not_expected", f"price={shelf_price},need={base_need}")
    require(delivery["utility_surcharge_per_q"] == 0, "fixture_has_unmodeled_household_utility_cost")
    goods_per_trip = partition["goods_per_trip"]

    for row, expected_employment, expected_day3 in ((c, "employment-control", 10), (x, "employment-change", 10)):
        require(row.get("mode") == "construction-utilities-three-day", "employment_case_wrong_joint_mode", row.get("mode", "missing"))
        require(int_field(row, "day1_capacity") == 2 and int_field(row, "day2_capacity") == 2, "base_road_capacity_wrong")
        require(int_field(row, "day3_capacity") == expected_day3, "day3_road_capacity_wrong")
        require(int_field(row, "day1_produced") == 100 and int_field(row, "day2_produced") == 0 and int_field(row, "day3_produced") == 0, "employment_production_basis_wrong")
        require(bool_field(row, "cash_conserved") and bool_field(row, "final_cold_open_equal"), "cash_or_cold_open_flag_false")
        require(bool_field(row, "employment_operation_replay_exact"), "employment_receipt_replay_not_exact")
        require(bool_field(row, "all_contract_payroll_and_market_receipts_replay_exact"), "contract_or_market_replay_not_exact")

    calculated = {}
    for row, world, mode_name in ((c, cworld, "control"), (x, xworld, "change")):
        wages = [80, 80, 80] if mode_name == "control" else [80, 8, 8]
        cash = 0
        inventory = 0
        sales = []
        freight_costs = []
        signatures = []
        for day in (1, 2, 3):
            proof_hash = row.get(f"day{day}_proof_sha256")
            require(proof_hash is not None and re.fullmatch(r"[0-9a-f]{64}", proof_hash) is not None, "proof_hash_missing", f"{mode_name}/day{day}")
            proof = load_proof(world, proof_hash, day)
            require(proof.get("world_id") == 1 and proof.get("branch_id") == 1 and proof.get("profile_id") == 1, "proof_identity_mismatch")
            require(bytes(proof.get("allocation_profile_sha256", [] )).hex() == profile_digest, "proof_profile_hash_mismatch")
            signature = transport_signature(proof)
            signatures.append(signature)
            edge10 = next((entry for entry in signature["flows"] if entry["edge_id"] == 10), None)
            edge11 = next((entry for entry in signature["flows"] if entry["edge_id"] == 11), None)
            require(edge10 is not None and edge11 is not None, "road_edges_missing", f"{mode_name}/day{day}")
            cap = int_field(row, f"day{day}_capacity")
            require(edge10["capacity_people"] == cap and edge10["people"] == min(10, cap), "outbound_capacity_or_flow_wrong", f"{mode_name}/day{day}")
            require(edge11["capacity_people"] == 10 and edge11["people"] == 10, "return_capacity_or_flow_wrong", f"{mode_name}/day{day}")
            require([d["source_id"] for d in signature["demands"]] == [101, 102], "wrong_fixed_od_sources")
            opening_edges = {edge["id"]: edge for edge in proof["opening"].get("edges", [])}
            require(opening_edges.get(10, {}).get("from") == 1 and opening_edges.get(10, {}).get("to") == 2, "outbound_edge_geometry_wrong")
            require(opening_edges.get(11, {}).get("from") == 2 and opening_edges.get(11, {}).get("to") == 1, "return_edge_geometry_wrong")
            outcomes_by_source = {d["source_id"]: d for d in proof["assignment"].get("demand_outcomes", [])}
            paths_by_source = {p["source_id"]: p for p in proof["assignment"].get("path_receipts", [])}
            require(paths_by_source.get(101, {}).get("edge_ids") == [10], "outbound_path_source_or_edge_wrong")
            require(paths_by_source.get(102, {}).get("edge_ids") == [11], "return_path_source_or_edge_wrong")
            require(paths_by_source[101]["people"] == outcomes_by_source[101]["served_people"], "outbound_path_people_mismatch")
            require(paths_by_source[102]["people"] == outcomes_by_source[102]["served_people"], "return_path_people_mismatch")
            entrances = {e["id"]: e for e in signature["entrances"]}
            require(entrances.get(1, {}).get("owner") == 5 and entrances.get(1, {}).get("authorized") is True, "household_entrance_not_authorized")
            require(entrances.get(2, {}).get("owner") == 7 and entrances.get(2, {}).get("authorized") is True, "factory_entrance_not_authorized")
            partition_rows = signature["partitions"] or []
            require(len(partition_rows) == 1, "proof_partition_count_wrong")
            require(partition_rows[0].get("trip_count") == partition["trip_count"] and partition_rows[0].get("goods_per_trip") == goods_per_trip, "proof_partition_differs_from_profile")

            outcomes = {entry["source_id"]: entry for entry in proof["assignment"].get("demand_outcomes", [])}
            outbound, inbound = outcomes.get(101), outcomes.get(102)
            require(outbound is not None and inbound is not None, "proof_trip_outcomes_missing")
            allocated_trips = min(partition["trip_count"], outbound["served_people"], inbound["served_people"])
            accessible_goods = allocated_trips * goods_per_trip
            fee_policy = profile.get("fee_policy", {})
            fee_rate = fee_policy.get("cents_per_travel_second")
            require(isinstance(fee_rate, int) and fee_rate >= 0, "transport_fee_policy_missing")
            if allocated_trips:
                round_trip_seconds = outbound["mean_travel_seconds"] + inbound["mean_travel_seconds"]
                total_freight_cents = round_trip_seconds * fee_rate * allocated_trips
                freight_per_unit = math.ceil(total_freight_cents / accessible_goods)
            else:
                freight_per_unit = 0
            utility_per_unit = delivery["utility_surcharge_per_q"]
            delivered_price = shelf_price + freight_per_unit + utility_per_unit
            require(delivered_price > 0, "nonpositive_delivered_price")
            demand = min(base_need, base_need * household["reference_price_per_q"] // delivered_price)

            produced = int_field(row, f"day{day}_produced")
            reported_sales = int_field(row, f"day{day}_sales")
            inventory += produced
            before_market = cash + wages[day - 1]
            route_goods = accessible_goods
            expected_sales = min(demand, inventory, route_goods, before_market // delivered_price)
            require(reported_sales == expected_sales, "daily_sales_not_independently_explained", f"{mode_name}/day{day}:actual={reported_sales},expected={expected_sales}")
            inventory -= reported_sales
            total_spend = reported_sales * delivered_price
            goods_spend = reported_sales * shelf_price
            freight_spend = reported_sales * freight_per_unit
            cash = before_market - total_spend
            sales.append(reported_sales)
            freight_costs.append(freight_spend)

            cash_field = {1: "day1_household_cash_after_market", 2: "day2_household_cash_after_market", 3: "day3_household_cash_after_market"}[day]
            require(int_field(row, cash_field) == cash, "daily_household_cash_equation_mismatch", f"{mode_name}/day{day}")
            if day == 3:
                require(int_field(row, "day3_household_cash_before_market") == before_market, "day3_pre_market_cash_mismatch", mode_name)
                require(int_field(row, "day3_goods_payment") == goods_spend, "day3_goods_spend_mismatch", mode_name)
                require(int_field(row, "day3_freight") == reported_sales * freight_per_unit, "day3_freight_spend_mismatch", mode_name)
                require(int_field(row, "day3_utilities") == reported_sales * utility_per_unit, "day3_utility_spend_mismatch", mode_name)
                require(int_field(row, "day3_market_spend") == total_spend, "day3_total_spend_mismatch", mode_name)
                require(int_field(row, "day3_stock_available") == inventory + reported_sales, "day3_opening_shelf_inventory_mismatch", mode_name)
        # Proof hashes can vary because receipt/world revisions differ. Compare the
        # normalized traffic facts only; do not assert hash equality.
        require(signatures[2]["edges"][0]["capacity_people"] == 10, "day3_proof_does_not_show_accepted_road")
        final_state = decode_store(world)[2]
        world_state = final_state.get("world", {})
        cash_rows = indexed_rows(world_state.get("accounts", {}))
        stock_rows = indexed_rows(world_state.get("stocks", {}))
        account_cash = {i: row["cash"] for i, row in cash_rows.items()}
        require(sum(account_cash.values()) == 3_101_000, "final_cash_total_not_conserved", mode_name)
        require(account_cash.get(5) == cash, "final_household_cash_differs_from_daily_equation", mode_name)
        require(account_cash.get(8) == int_field(row, "new_employer_cash_final"), "new_employer_cash_report_mismatch", mode_name)
        require(stock_rows.get(16, {}).get("quantity") == inventory, "final_product_stock_not_conserved", mode_name)
        require(stock_rows.get(15, {}).get("quantity") == 400, "final_raw_stock_not_conserved", mode_name)
        require(stock_rows.get(2, {}).get("quantity") == 700_000, "final_electricity_stock_wrong", mode_name)
        project = world_state.get("construction", {})
        used_materials = [m for m in project.get("materials", []) if m.get("stock_id") == 15]
        require(len(used_materials) == 1 and used_materials[0].get("consumed") == 500, "construction_material_consumption_wrong", mode_name)
        opening_raw = next((s for s in opening.get("stocks", []) if s.get("id") == 15), None)
        require(opening_raw is not None and opening_raw.get("quantity") == 1000, "opening_raw_quantity_wrong")
        require(opening_raw["quantity"] == stock_rows[15]["quantity"] + used_materials[0]["consumed"] + 100, "raw_material_factory_construction_conservation_wrong", mode_name)
        require(project.get("phase") == "operational" and project.get("operating_published") is True and project.get("operating_published_at") == 172800, "construction_acceptance_boundary_wrong", mode_name)
        require(project.get("completed_worker_seconds") == 28_800 and project.get("utility_consumed", [])[:2] == [400_000, 400_000], "construction_work_or_utility_basis_wrong", mode_name)
        require(project.get("utility_charged", [])[:2] == [8_000, 4_000] and project.get("spent") == 41_300, "construction_cost_basis_wrong", mode_name)
        boot = world_state.get("boot", {})
        require(boot.get("local_electricity_exported") == 100_000 and boot.get("local_electricity_basis") == 182, "boot_local_market_export_wrong", mode_name)
        require(boot.get("local_construction_exported", [])[:2] == [400_000, 400_000], "boot_construction_exports_wrong", mode_name)
        resource = next((r for r in final_state.get("resources", []) if r.get("id") == 81), None)
        require(resource is not None and resource.get("capacity") == 10, "final_road_resource_not_published", mode_name)
        timeline = final_state.get("timeline", {})
        payrolls = {p.get("spec", {}).get("id"): p for p in timeline.get("payroll", [])}
        old = payrolls.get(1)
        new = payrolls.get(2)
        require(old is not None, "old_payroll_missing", mode_name)
        if mode_name == "control":
            require(new is None, "control_has_unexpected_new_payroll")
            require(old["spec"]["payer"] == 7 and old["work_seconds"] == 86_400 and old["paid"] == 240, "control_old_payroll_wrong")
            require(int_field(row, "old_factory_paid") == 240 and int_field(row, "new_employer_paid") == 0, "control_payroll_report_wrong")
        else:
            require(new is not None, "change_new_payroll_missing")
            require(old["spec"]["payer"] == 7 and old["spec"]["ends_at"] == 86_400 and old["work_seconds"] == 28_800 and old["paid"] == 80, "old_job_not_terminated_at_switch")
            require(new["spec"]["payer"] == 8 and new["spec"]["payee"] == 5 and new["started"] == 86_400 and new["work_seconds"] == 57_600 and new["paid"] == 16, "new_job_or_wage_wrong")
            require(old["spec"]["ends_at"] == new["started"], "employment_interval_overlap_or_gap")
            require(int_field(row, "old_factory_paid") == 80 and int_field(row, "new_employer_paid") == 16, "change_payroll_report_wrong")
        expected_total_paid = 240 if mode_name == "control" else 96
        require(int_field(row, "payroll_paid") == expected_total_paid, "payroll_paid_total_wrong", mode_name)
        calculated[mode_name] = {
            "sales": sum(sales),
            "goods_cents": sum(sales) * shelf_price,
            "freight_cents": sum(freight_costs),
            "consumer_spend_cents": sum(sales) * delivered_price,
        }

    control_signatures = [
        transport_signature(load_proof(cworld, c[f"day{day}_proof_sha256"], day)) for day in (1, 2, 3)
    ]
    change_signatures = [
        transport_signature(load_proof(xworld, x[f"day{day}_proof_sha256"], day)) for day in (1, 2, 3)
    ]
    require(control_signatures == change_signatures, "control_change_traffic_structure_differs")
    require(int_field(c, "day3_sales") == 20 and int_field(x, "day3_sales") == 2, "day3_market_result_not_implied_by_cash_and_road")

    c_state = decode_store(cworld)[2]["world"]
    x_state = decode_store(xworld)[2]["world"]
    c_cash = {i: row["cash"] for i, row in indexed_rows(c_state["accounts"]).items()}
    x_cash = {i: row["cash"] for i, row in indexed_rows(x_state["accounts"]).items()}
    sales_control = sum(int_field(c, f"day{day}_sales") for day in (1, 2, 3))
    sales_change = sum(int_field(x, f"day{day}_sales") for day in (1, 2, 3))
    require(c_cash.get(7, 0) - x_cash.get(7, 0) == (sales_control - sales_change) * shelf_price - (240 - 80), "company7_cash_difference_not_explained_by_sales_and_old_wages")
    require(c_cash.get(1, 0) - x_cash.get(1, 0) == calculated["control"]["freight_cents"] - calculated["change"]["freight_cents"], "freight_provider_cash_difference_not_explained")
    require(c_cash.get(8, 0) - x_cash.get(8, 0) == 16, "new_employer_cash_difference_not_explained")
    require(c_cash.get(5, 0) == x_cash.get(5, 0) == 0, "household_pair_final_cash_not_equal")
    return {
        "shelf_price_cents_per_unit": shelf_price,
        "effective_delivered_unit_price_derived_per_day": 4,
        "computed_demand_units_per_day": base_need,
        "goods_per_trip": goods_per_trip,
        "control_daily_wages_cents": [80, 80, 80],
        "change_daily_wages_cents": [80, 8, 8],
        "control_sales_by_day": [int_field(c, f"day{d}_sales") for d in (1, 2, 3)],
        "change_sales_by_day": [int_field(x, f"day{d}_sales") for d in (1, 2, 3)],
        "control_goods_and_freight_cents": {"goods": calculated["control"]["goods_cents"], "freight": calculated["control"]["freight_cents"], "total": calculated["control"]["consumer_spend_cents"]},
        "change_goods_and_freight_cents": {"goods": calculated["change"]["goods_cents"], "freight": calculated["change"]["freight_cents"], "total": calculated["change"]["consumer_spend_cents"]},
        "traffic_structures_match_without_hash_equality": True,
        "control_final_cash_and_inventory_checked_from_cold_checkpoint": True,
        "change_final_cash_and_inventory_checked_from_cold_checkpoint": True,
        "relative_employer_and_freight_provider_cashflows_reconciled": True,
        "scope_limits": {
            "employment_assignment": "trusted_fixture_input",
            "endogenous_job_search": "not_implemented",
            "legal_employment_approval": "not_modeled",
            "transport_od": "fixed_sources_101_102",
            "continuous_self_funding": "not_demonstrated",
        },
        "known_cli_field_semantics": {
            "market_payment_total": "not_used_as_goods_sales; it sums the utility operator payment returned by settle_three_day_market",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", type=Path, default=DEFAULT_RUN)
    parser.add_argument("--history-root", type=Path, default=DEFAULT_HISTORY)
    parser.add_argument("--fixture", type=Path, default=Path("/Users/frankqin/dev/MyCity-core-work/employment-lifelines-current/fixtures/core-town-utility-opening.json"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result: dict[str, Any] = {
        "scope": "independent_functional_case_audit",
        "model_or_cargo_executed": False,
        "performance_acceptance": "not_in_scope",
        "checks": [],
        "failures": [],
        "missing_evidence": [],
    }
    try:
        cases = load_case_records(args.run_root)
        missing = EXPECTED_CASES - set(cases)
        require(not missing, "required_cases_incomplete", ",".join(sorted(missing)))
        extra = set(cases) - EXPECTED_CASES
        require(not extra, "unexpected_case_modes", ",".join(sorted(extra)))
        current = {}
        for case in sorted(EXPECTED_CASES):
            mode, scope, world, _ = checked_case(cases, case)
            current[case] = (mode, scope, world, cases[case][0])
        result["historical_five_mode_regression"] = verify_historical_regressions(cases, args.history_root)
        control = checked_case(cases, "employment-control")
        change = checked_case(cases, "employment-change")
        result["common_opening"] = verify_opening_pair(control[2], change[2], args.fixture)
        result["employment_economy_and_traffic"] = verify_employment_pair(control, change, args.fixture)
        result["checks"] = [
            "seven case receipts have process exit 0 and successful mode/scope output",
            "five pre-existing modes preserve 108 business fields and exact state hashes",
            "employment control/change full genesis.json and embedded opening bytes match exactly",
            "every account cash and stock basis is backed by matching opening source",
            "daily sales are recomputed from demand, cash, inventory, fixed quote price, transport fee, and proof-backed route capacity",
            "old/new payroll intervals, work seconds, accrued/paid wages, household cash, market spend and final inventory reconcile",
            "traffic proof IDs, entrances, directions, route paths, capacities, profile identity, and cold checkpoint are checked",
            "employment comparisons do not require equal proof hashes; normalized daily traffic structure is compared",
        ]
        result["status"] = "passed" if not result["failures"] else "failed"
    except EvidenceError as exc:
        result["status"] = "incomplete_or_failed"
        message = str(exc)
        if message.startswith("missing:") or message.startswith("missing_case:") or message.startswith("required_cases_incomplete:") or message.startswith("case_directory_missing:") or message.startswith("missing_stdout:"):
            result["missing_evidence"].append(message)
        else:
            result["failures"].append(message)
    except Exception as exc:  # retain an explicit error; never convert it to success
        result["status"] = "validator_error"
        result["failures"].append(f"{type(exc).__name__}:{exc}")
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded)
    print(encoded, end="")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
