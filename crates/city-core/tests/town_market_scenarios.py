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


def check_conservation(row: dict[str, str]) -> None:
    assert row["live_transport"] == "false"
    assert row["live_utility"] == "false"
    assert row["tax_policy"] == "not_configured"
    assert number(row, "initial_cash_total") == number(row, "final_cash_total")
    assert number(row, "raw_consumed") == number(row, "product_produced") + number(row, "waste")
    assert number(row, "product_produced") == number(row, "product_sold") + number(row, "product_remaining")
    assert number(row, "wages_accrued") == number(row, "capitalized_labor_basis") + number(row, "idle_labor_expense")
    assert number(row, "debt_principal") == number(row, "debt_paid") + number(row, "debt_outstanding")


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
    for row in cases.values():
        check_conservation(row)
    check_conservation(parameterized)

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

    print("town_market_scenarios=passed scenarios=7 parameterized=1 cli=real_engine_and_timeline")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
