#!/usr/bin/env python3
"""Run the preserved 28-day town_market CLI matrix through run-command.py.

This driver is intentionally not executed during authoring. It uses the saved
31-cli-build executable, verifies its binary/source pins first, then delegates
every child launch to the existing evidence wrapper. The wrapper's own exit
status is not the child's status; each exit.json is checked explicitly.

These are functional Engine/Timeline profiles. Transport reachability/capacity
and utility cost are fixed profile substitutes, not live networks. The fixtures
start with registered capital; sustainable self-funding and profitability are
not demonstrated. fixture_turnover is an uncalibrated example tax claim only.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import re
import secrets
import subprocess
import sys
from pathlib import Path
from typing import Any


WORK_ROOT = Path("/Volumes/HP P900/dev/MyCity/work/core-road-town-precommit-01")
REPO = Path("/Users/frankqin/dev/MyCity-core-work/production")
BUILD_DIR = WORK_ROOT / "31-cli-build"
BUILD_RECEIPT = BUILD_DIR / "exit.json"
BUILD_COMMAND = BUILD_DIR / "command.json"
BUILD_MANIFEST = BUILD_DIR / "source-manifest.json"
CLI_RECEIPT = BUILD_DIR / "cli-executables.json"
RUN_COMMAND = WORK_ROOT / "run-command.py"
MARKET_SOURCE = REPO / "crates/city-core/examples/town_market.rs"
MARKET_SOURCE_SHA256 = "dd27bf861d0d144b839b929da37d56c496f031c1261679af3f35a4393a49404a"
MARKET_BINARY = WORK_ROOT / "executables/e97c19648f3346f13b572758da30c9a07e5d75685a0233bbf457bd0913825b88/town_market"
MARKET_BINARY_SHA256 = "e97c19648f3346f13b572758da30c9a07e5d75685a0233bbf457bd0913825b88"
TIME_TOOL = Path("/usr/bin/time")
TARGET_ROOT = Path("/Volumes/HP P900/dev/MyCity/target")

SCENARIOS = (
    "base",
    "cash-shortage",
    "raw-shortage",
    "no-route",
    "capacity-cut",
    "energy-price-rise",
    "demand-drop",
)


class ValidationError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def number(fields: dict[str, str], key: str) -> int:
    try:
        return int(fields[key])
    except (KeyError, ValueError) as error:
        raise ValidationError(f"missing_or_invalid_integer:{key}") from error


def native_time_metrics(stderr: str) -> dict[str, int | None]:
    """Parse macOS /usr/bin/time -l byte counts without mixing RSS and footprint."""
    metrics: dict[str, int | None] = {}
    for key, label in (
        ("max_rss_bytes", "maximum resident set size"),
        ("peak_footprint_bytes", "peak memory footprint"),
    ):
        rows = re.findall(rf"^[ \t]*([0-9]+)[ \t]+{re.escape(label)}[ \t]*$", stderr, re.MULTILINE)
        require(len(rows) <= 1, f"ambiguous_native_time_metric:{key}")
        metrics[key] = int(rows[0]) if rows else None
    require(metrics["max_rss_bytes"] is not None and metrics["max_rss_bytes"] > 0, "native_time_rss_missing_or_invalid")
    return metrics


def verify_saved_binary() -> dict[str, Any]:
    for path in (BUILD_RECEIPT, BUILD_COMMAND, BUILD_MANIFEST, CLI_RECEIPT, RUN_COMMAND, MARKET_SOURCE, MARKET_BINARY, TIME_TOOL):
        require(path.is_file() and not path.is_symlink(), f"required_regular_file_missing:{path}")

    build = json.loads(BUILD_RECEIPT.read_text(encoding="utf-8"))
    build_command = json.loads(BUILD_COMMAND.read_text(encoding="utf-8"))
    require(build.get("exit_code") == 0, "31-cli-build_child_failed")
    require(build.get("source_unchanged") is True, "31-cli-build_source_changed")
    require(build_command.get("source_manifest_sha256") == sha256(BUILD_MANIFEST), "31-cli-build_manifest_hash_mismatch")

    manifest_rows = json.loads(BUILD_MANIFEST.read_text(encoding="utf-8"))
    market_rows = [row for row in manifest_rows if row.get("path") == "crates/city-core/examples/town_market.rs"]
    require(len(market_rows) == 1, "31-cli-build_missing_market_source_pin")
    require(market_rows[0].get("sha256") == MARKET_SOURCE_SHA256, "31-cli-build_unexpected_market_source_hash")
    require(sha256(MARKET_SOURCE) == MARKET_SOURCE_SHA256, "current_market_source_differs_from_saved_executable_source")

    executable_manifest = json.loads(CLI_RECEIPT.read_text(encoding="utf-8"))
    market = executable_manifest.get("town_market")
    require(isinstance(market, dict), "31-cli-build_missing_town_market_executable_receipt")
    require(Path(str(market.get("preserved"))) == MARKET_BINARY, "31-cli-build_saved_binary_path_mismatch")
    require(market.get("sha256") == MARKET_BINARY_SHA256, "31-cli-build_binary_receipt_hash_mismatch")
    actual_binary_hash = sha256(MARKET_BINARY)
    require(actual_binary_hash == MARKET_BINARY_SHA256, "31-cli-build_saved_binary_hash_mismatch")
    require(bool(MARKET_BINARY.stat().st_mode & 0o111), "saved_town_market_binary_not_executable")
    return {
        "build_dir": str(BUILD_DIR),
        "build_exit": build.get("exit_code"),
        "build_source_unchanged": build.get("source_unchanged"),
        "build_source_manifest_sha256": build_command.get("source_manifest_sha256"),
        "market_source_sha256": MARKET_SOURCE_SHA256,
        "market_binary": str(MARKET_BINARY),
        "market_binary_sha256": actual_binary_hash,
        "driver_sha256": sha256(Path(__file__).resolve()),
        "run_command_sha256": sha256(RUN_COMMAND),
        "native_time_tool": str(TIME_TOOL),
    }


def command_matrix() -> list[tuple[str, list[str], str | None, bool]]:
    binary = str(MARKET_BINARY)
    rows: list[tuple[str, list[str], str | None, bool]] = [
        (f"town-market-{scenario}", [binary, "--scenario", scenario], scenario, True)
        for scenario in SCENARIOS
    ]
    rows.extend(
        [
            (
                "town-market-turnover-tax-500bps",
                [binary, "--scenario", "base", "--turnover-tax-bps", "500"],
                "base",
                True,
            ),
            (
                "town-market-turnover-tax-cash-pressure-500bps",
                [
                    binary,
                    "--scenario",
                    "base",
                    "--company-cash",
                    "0",
                    "--debt-principal",
                    "1000000",
                    "--debt-due-day",
                    "2",
                    "--production-utility-cost",
                    "50",
                    "--turnover-tax-bps",
                    "500",
                ],
                "base",
                True,
            ),
            # These two checks are in town_market_scenarios.py, but not in the
            # controller's nine-command release matrix.
            (
                "town-market-parameter-override",
                [binary, "--scenario", "base", "--route-capacity", "2", "--daily-need", "5"],
                "base",
                True,
            ),
            (
                "town-market-reject-tax-over-limit",
                [binary, "--turnover-tax-bps", "10001"],
                None,
                False,
            ),
        ]
    )
    return rows


def parse_summary(stdout: str, expected_scenario: str) -> dict[str, str]:
    candidates = [
        line.strip()
        for line in stdout.splitlines()
        if re.search(r"(?:^|\s)scenario=", line)
    ]
    require(len(candidates) == 1, "town_market_must_emit_one_scenario_summary")
    matches = list(re.finditer(r"(?:^|\s)([a-z_]+)=(.*?)(?=\s[a-z_]+=|$)", candidates[0]))
    fields = {match.group(1): match.group(2).strip() for match in matches}
    require(len(fields) == len(matches), "town_market_summary_duplicate_key")
    require(fields.get("scenario") == expected_scenario, "town_market_scenario_mismatch")
    require(fields.get("days") == "28", "town_market_not_28_days")
    require(fields.get("live_transport") == "false", "town_market_claimed_live_transport")
    require(fields.get("live_utility") == "false", "town_market_claimed_live_utility")
    require(fields.get("tax_policy") == "not_configured", "town_market_tax_scope_changed")
    return fields


def check_conservation(row: dict[str, str]) -> None:
    require(number(row, "initial_cash_total") == number(row, "final_cash_total"), "cash_not_conserved")
    require(number(row, "raw_consumed") == number(row, "product_produced") + number(row, "waste"), "raw_balance_mismatch")
    require(number(row, "product_produced") == number(row, "product_sold") + number(row, "product_remaining"), "product_balance_mismatch")
    require(number(row, "wages_accrued") == number(row, "capitalized_labor_basis") + number(row, "idle_labor_expense"), "labor_cost_balance_mismatch")
    require(number(row, "wages_accrued") == number(row, "wages_paid") + number(row, "wages_arrears"), "wage_payment_balance_mismatch")
    require(number(row, "debt_principal") == number(row, "debt_paid") + number(row, "debt_outstanding"), "debt_balance_mismatch")
    require(number(row, "tax_assessed") == number(row, "tax_paid") + number(row, "tax_outstanding"), "tax_balance_mismatch")
    require(number(row, "tax_future_due") <= number(row, "tax_outstanding"), "future_tax_exceeds_outstanding")
    treasury_delta = number(row, "treasury_cash_final") - number(row, "treasury_cash_initial")
    expected_delta = (
        -number(row, "debt_principal")
        + number(row, "debt_paid")
        + number(row, "freight")
        + number(row, "utilities")
        + number(row, "disposal")
        + number(row, "tax_paid")
    )
    require(treasury_delta == expected_delta, "treasury_cash_flow_mismatch")


def check_scenario_invariants(results: dict[str, dict[str, str]]) -> None:
    for row in results.values():
        check_conservation(row)

    base = results["base"]
    require(number(base, "product_sold") > 0, "base_has_no_sales")
    require(number(base, "wages_paid") > 0, "base_has_no_wage_payments")
    require(number(base, "debt_paid") > 0, "base_has_no_debt_repayment")
    require(number(results["cash-shortage"], "starting_household_cash") == 0, "cash_shortage_fixture_not_applied")
    require(number(results["cash-shortage"], "product_sold") < number(base, "product_sold"), "cash_shortage_did_not_reduce_sales")
    require(number(results["cash-shortage"], "unmet_market_demand") > number(base, "unmet_market_demand"), "cash_shortage_did_not_raise_unmet_demand")
    require(0 < number(results["raw-shortage"], "product_produced") <= 15, "raw_shortage_production_out_of_range")
    require(number(results["raw-shortage"], "raw_consumed") <= 15, "raw_shortage_consumed_unavailable_input")
    require(number(results["no-route"], "product_sold") == 0, "no_route_sold_goods")
    require(number(results["no-route"], "reference_need") > 0, "no_route_lost_reference_need")
    require(number(results["capacity-cut"], "product_sold") <= 3 * 28, "capacity_cut_exceeded_capacity")
    require(number(results["capacity-cut"], "product_sold") < number(base, "product_sold"), "capacity_cut_did_not_reduce_sales")
    require(number(results["energy-price-rise"], "utilities") > number(base, "utilities"), "energy_price_did_not_raise_cost")
    require(number(results["energy-price-rise"], "producer_cash_final") < number(base, "producer_cash_final"), "energy_price_did_not_reduce_cash")
    require(number(results["demand-drop"], "market_demand") < number(base, "market_demand"), "demand_drop_not_applied")
    require(number(results["demand-drop"], "product_sold") < number(base, "product_sold"), "demand_drop_did_not_reduce_sales")

    parameterized = results["parameterized"]
    require(number(parameterized, "configured_daily_need") == 5, "daily_need_override_not_applied")
    require(number(parameterized, "configured_route_capacity") == 2, "route_capacity_override_not_applied")
    require(number(parameterized, "product_sold") <= 2 * 28, "parameterized_sales_exceeded_capacity")

    untaxed = base
    require(number(untaxed, "tax_assessed") == number(untaxed, "tax_paid") == 0, "untaxed_base_assessed_tax")
    require(untaxed.get("turnover_tax_fixture") == "none", "untaxed_fixture_label_mismatch")

    tax = results["tax"]
    require(tax.get("turnover_tax_fixture") == "fixture_turnover", "tax_fixture_label_mismatch")
    require(number(tax, "turnover_tax_bps") == 500, "tax_rate_mismatch")
    require(number(tax, "tax_payer_account_id") == 3 and number(tax, "tax_authority_account_id") == 1, "tax_parties_mismatch")
    require(number(tax, "tax_basis_goods_payments") == number(tax, "goods_payments"), "tax_basis_not_actual_goods_payments")
    require(number(tax, "tax_assessed") == number(tax, "tax_basis_goods_payments") * 500 // 10_000, "tax_assessment_mismatch")
    require(number(tax, "tax_paid") > 0 and number(tax, "tax_outstanding") > 0, "normal_tax_did_not_show_paid_and_outstanding")
    require(number(tax, "tax_future_due") == number(tax, "tax_outstanding"), "day_29_tax_not_all_future_due")
    require(number(tax, "tax_next_due_day") == 29 and number(tax, "tax_future_due_claims") > 0, "day_29_tax_due_missing")
    require(number(tax, "treasury_cash_final") - number(base, "treasury_cash_final") == number(tax, "tax_paid"), "tax_treasury_delta_mismatch")
    require(number(base, "producer_cash_final") - number(tax, "producer_cash_final") == number(tax, "tax_paid"), "tax_company_delta_mismatch")

    pressure = results["cash_pressure_tax"]
    require(number(pressure, "product_sold") == 100, "cash_pressure_sales_mismatch")
    require(number(pressure, "tax_assessed") == 500, "cash_pressure_tax_assessed_mismatch")
    require(number(pressure, "tax_paid") == 450, "cash_pressure_tax_paid_mismatch")
    require(number(pressure, "tax_outstanding") == 50 and number(pressure, "tax_future_due") == 0, "cash_pressure_overdue_tax_mismatch")
    require(number(pressure, "tax_next_due_day") == 0, "cash_pressure_tax_still_future_due")
    require(number(pressure, "wages_paid") == 8_800 and number(pressure, "wages_arrears") == 13_600, "cash_pressure_wages_mismatch")
    require(number(pressure, "debt_paid") == 995_750 and number(pressure, "debt_outstanding") == 4_250, "cash_pressure_debt_mismatch")


def main() -> int:
    try:
        binary_receipt = verify_saved_binary()
        run_id = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + secrets.token_hex(5)
        matrix = command_matrix()
        labels = [f"economy-cli-{run_id}-{label}" for label, _, _, _ in matrix]
        destinations = [WORK_ROOT / label for label in labels]
        summary_path = WORK_ROOT / f"economy-cli-validator-{run_id}.json"
        require(WORK_ROOT.is_dir(), f"external_work_root_missing:{WORK_ROOT}")
        require(REPO.is_dir(), f"production_repo_missing:{REPO}")
        require(all(not path.exists() for path in destinations), "evidence_label_collision")
        require(not summary_path.exists(), "summary_path_collision")

        results: dict[str, dict[str, str]] = {}
        run_records: list[dict[str, Any]] = []
        for (label, argv, expected_scenario, expected_success), destination in zip(matrix, destinations):
            evidence_label = destination.name
            timed_argv = [str(TIME_TOOL), "-l", *argv]
            wrapper = subprocess.run(
                [sys.executable, str(RUN_COMMAND), evidence_label, *timed_argv],
                cwd=REPO,
                capture_output=True,
                text=True,
                check=False,
            )
            receipt_path = destination / "exit.json"
            require(receipt_path.is_file(), f"run_wrapper_did_not_write_exit_receipt:{evidence_label}")
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            child_exit = receipt.get("exit_code")
            require(receipt.get("source_unchanged") is True, f"wrapper_source_changed:{evidence_label}")
            require(wrapper.returncode == 0, f"run_wrapper_failed:{evidence_label}:exit={wrapper.returncode}")
            require(isinstance(child_exit, int), f"child_exit_missing:{evidence_label}")
            require((child_exit == 0) == expected_success, f"unexpected_child_exit:{evidence_label}:{child_exit}")

            command_receipt = json.loads((destination / "command.json").read_text(encoding="utf-8"))
            require(command_receipt.get("argv") == timed_argv, f"command_receipt_argv_mismatch:{evidence_label}")
            require(command_receipt.get("target") == str(TARGET_ROOT), f"shared_target_not_on_external_ssd:{evidence_label}")
            stdout_path = destination / "stdout.log"
            stderr_path = destination / "stderr.log"
            require(stdout_path.is_file() and stderr_path.is_file(), f"child_logs_missing:{evidence_label}")
            runtime_metrics = native_time_metrics(stderr_path.read_text(encoding="utf-8", errors="replace"))
            fields: dict[str, str] | None = None
            if expected_success:
                fields = parse_summary(stdout_path.read_text(encoding="utf-8"), expected_scenario or "")
                results[label] = fields
            else:
                error_output = stderr_path.read_text(encoding="utf-8", errors="replace")
                require("invalid_scenario_values_or_turnover_tax_bps_over_10000" in error_output, "invalid_tax_rate_rejected_for_wrong_reason")
            run_records.append({
                "case": label,
                "argv": argv,
                "expected_success": expected_success,
                "child_exit_code": child_exit,
                "evidence_dir": str(destination),
                "work_volume": "/Volumes/HP P900 (external SSD)",
                "target_root": str(TARGET_ROOT),
                "runtime_metrics": runtime_metrics,
                "summary_fields": fields,
            })
            print(f"{label}: child_exit={child_exit} evidence={destination}", flush=True)

        named_results = {scenario: results[f"town-market-{scenario}"] for scenario in SCENARIOS}
        named_results["parameterized"] = results["town-market-parameter-override"]
        named_results["tax"] = results["town-market-turnover-tax-500bps"]
        named_results["cash_pressure_tax"] = results["town-market-turnover-tax-cash-pressure-500bps"]
        check_scenario_invariants(named_results)

        summary = {
            "schema": "mycity-town-market-external-cli-validator-v1",
            "run_id": run_id,
            "binary_receipt": binary_receipt,
            "driver_sha256": binary_receipt["driver_sha256"],
            "run_command_sha256": binary_receipt["run_command_sha256"],
            "execution_storage": {
                "work_volume": "/Volumes/HP P900 (external SSD)",
                "work_root": str(WORK_ROOT),
                "shared_target": str(TARGET_ROOT),
                "target_verified_from_command_receipts": True,
            },
            "native_time_metrics_scope": "whole town_market CLI process; RSS and peak footprint are separate; this functional matrix is not M0 performance acceptance",
            "matrix_source": "run_controller.py town_market_commands plus its town_market_scenarios.py parameterized and rejection checks",
            "scenario_scope": "real Engine plus Timeline over 28 game days; fixed transport and utility profile substitutes; live_transport=false; live_utility=false",
            "economic_scope": "registered initial fixture capital; no claim of sustainable self-funding, calibrated profitability, or complete tax policy",
            "tax_scope": "fixture_turnover only; 500 bps against actual TownSettle goods payments, ordinary Timeline claims to Treasury",
            "runs": run_records,
            "result": "all checks passed",
        }
        with summary_path.open("x", encoding="utf-8") as stream:
            json.dump(summary, stream, indent=2, sort_keys=True)
            stream.write("\n")
        print(f"summary={summary_path}", flush=True)
        return 0
    except (ValidationError, OSError, ValueError, KeyError) as error:
        print(f"VALIDATION FAILED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
