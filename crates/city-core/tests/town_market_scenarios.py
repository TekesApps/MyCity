#!/usr/bin/env python3
"""Behavior checks for the built town_market example; invokes the real Engine CLI."""

import re
import subprocess
import sys
from pathlib import Path


def run(binary: Path, scenario: str, *overrides: str) -> dict[str, str]:
    result = subprocess.run(
        [str(binary), "--scenario", scenario, *overrides],
        check=True,
        capture_output=True,
        text=True,
    )
    line = result.stdout.splitlines()[0]
    return dict(re.findall(r"(?:^|\s)([a-z_]+)=([^\s]+)", line))


def number(row: dict[str, str], key: str) -> int:
    return int(row[key])


def rejects(binary: Path, *args: str) -> None:
    result = subprocess.run([str(binary), *args], capture_output=True, text=True)
    assert result.returncode != 0


def check_conservation(row: dict[str, str]) -> None:
    assert row["live_transport"] == "false"
    assert row["live_utility"] == "false"
    assert row["tax_policy"] == "not_configured"
    assert number(row, "initial_cash_total") == number(row, "final_cash_total")
    assert number(row, "raw_consumed") == number(row, "product_produced") + number(row, "waste")
    assert number(row, "product_produced") == number(row, "product_sold") + number(row, "product_remaining")
    assert number(row, "wages_accrued") == number(row, "capitalized_labor_basis") + number(row, "idle_labor_expense")
    assert number(row, "wages_accrued") == number(row, "wages_paid") + number(row, "wages_arrears")
    assert number(row, "debt_principal") == number(row, "debt_paid") + number(row, "debt_outstanding")
    assert number(row, "tax_assessed") == number(row, "tax_paid") + number(row, "tax_outstanding")
    assert number(row, "tax_future_due") <= number(row, "tax_outstanding")
    assert number(row, "treasury_cash_final") - number(row, "treasury_cash_initial") == (
        -number(row, "debt_principal")
        + number(row, "debt_paid")
        + number(row, "freight")
        + number(row, "utilities")
        + number(row, "disposal")
        + number(row, "tax_paid")
    )


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: town_market_scenarios.py /path/to/town_market")
    binary = Path(sys.argv[1]).resolve(strict=True)
    cases = {
        name: run(binary, name)
        for name in (
            "base",
            "cash-shortage",
            "raw-shortage",
            "no-route",
            "capacity-cut",
            "energy-price-rise",
            "demand-drop",
        )
    }
    parameterized = run(
        binary,
        "base",
        "--route-capacity",
        "2",
        "--daily-need",
        "5",
    )
    base_tax = run(binary, "base", "--turnover-tax-bps", "500")
    low_cash_overrides = (
        "--company-cash",
        "0",
        "--debt-principal",
        "1000000",
        "--debt-due-day",
        "2",
        "--production-utility-cost",
        "50",
    )
    low_cash_tax = run(
        binary,
        "base",
        *low_cash_overrides,
        "--turnover-tax-bps",
        "500",
    )
    rejects(binary, "--turnover-tax-bps", "10001")
    for row in cases.values():
        check_conservation(row)
    check_conservation(parameterized)
    check_conservation(base_tax)
    check_conservation(low_cash_tax)

    base = cases["base"]
    assert number(base, "days") == 28
    assert number(base, "product_sold") > 0
    assert number(base, "wages_paid") > 0
    assert number(base, "debt_paid") > 0
    assert number(cases["cash-shortage"], "starting_household_cash") == 0
    assert number(cases["cash-shortage"], "product_sold") < number(base, "product_sold")
    assert number(cases["cash-shortage"], "unmet_market_demand") > number(base, "unmet_market_demand")
    assert 0 < number(cases["raw-shortage"], "product_produced") <= 15
    assert number(cases["raw-shortage"], "raw_consumed") <= 15
    assert number(cases["no-route"], "product_sold") == 0
    assert number(cases["no-route"], "reference_need") > 0
    assert number(cases["capacity-cut"], "product_sold") <= 3 * 28
    assert number(cases["capacity-cut"], "product_sold") < number(base, "product_sold")
    assert number(cases["energy-price-rise"], "utilities") > number(base, "utilities")
    assert number(cases["energy-price-rise"], "producer_cash_final") < number(base, "producer_cash_final")
    assert number(cases["demand-drop"], "market_demand") < number(base, "market_demand")
    assert number(cases["demand-drop"], "product_sold") < number(base, "product_sold")
    assert number(parameterized, "configured_daily_need") == 5
    assert number(parameterized, "configured_route_capacity") == 2
    assert number(parameterized, "product_sold") <= 2 * 28
    assert number(base, "tax_assessed") == number(base, "tax_paid") == 0
    assert base["turnover_tax_fixture"] == "none"
    assert base_tax["turnover_tax_fixture"] == "fixture_turnover"
    assert number(base_tax, "turnover_tax_bps") == 500
    assert number(base_tax, "tax_payer_account_id") == 3
    assert number(base_tax, "tax_authority_account_id") == 1
    assert number(base_tax, "tax_basis_goods_payments") == number(base_tax, "goods_payments")
    assert number(base_tax, "tax_assessed") == (
        number(base_tax, "tax_basis_goods_payments") * 500 // 10_000
    )
    assert number(base_tax, "tax_paid") > 0
    assert number(base_tax, "tax_outstanding") > 0
    assert number(base_tax, "tax_future_due") == number(base_tax, "tax_outstanding")
    assert number(base_tax, "tax_next_due_day") == 29
    assert number(base_tax, "tax_future_due_claims") > 0
    assert number(base_tax, "treasury_cash_final") - number(base, "treasury_cash_final") == number(
        base_tax, "tax_paid"
    )
    assert number(base, "producer_cash_final") - number(base_tax, "producer_cash_final") == number(
        base_tax, "tax_paid"
    )
    assert number(low_cash_tax, "product_sold") == 100
    assert number(low_cash_tax, "tax_assessed") == 500
    assert number(low_cash_tax, "tax_paid") == 450
    assert number(low_cash_tax, "tax_outstanding") == 50
    assert number(low_cash_tax, "tax_future_due") == 0
    assert number(low_cash_tax, "tax_next_due_day") == 0
    assert number(low_cash_tax, "wages_paid") == 8_800
    assert number(low_cash_tax, "wages_arrears") == 13_600
    assert number(low_cash_tax, "debt_paid") == 995_750
    assert number(low_cash_tax, "debt_outstanding") == 4_250

    print("town_market_scenarios=passed scenarios=7 parameterized=1 tax_cases=2 cli=real_engine_and_timeline")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
