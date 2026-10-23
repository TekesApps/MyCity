#[path = "../examples/support/town_equity_28d.rs"]
mod scenario;

use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "town-equity-28d-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        eprintln!("TOWN_EQUITY_28D_ARTIFACT={}", path.display());
        Self(fs::canonicalize(path).unwrap())
    }
    fn world(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!("TOWN_EQUITY_28D_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn value_i64(value: &Value, key: &str) -> i64 {
    value[key].as_i64().unwrap()
}

#[test]
fn base_28_day_real_margin_tax_dividend_fiscal_use_and_exit_status() {
    let temp = Temp::new();
    let report = scenario::run(&temp.world("base"), scenario::Scenario::Base).unwrap();
    assert_eq!(report["sales_total"], 280);
    assert_eq!(report["production_total"], 280);
    assert_eq!(report["raw_remaining"], 0);
    assert_eq!(report["product_remaining"], 0);
    assert_eq!(report["seller_payroll_paid_cents"], 22_400);
    assert_eq!(report["tax_assessed_cents"], 1_260);
    assert_eq!(report["tax_paid_cents"], 1_260);
    assert_eq!(report["tax_claim_count"], 28);
    assert_eq!(report["initial_total_cash_cents"], 3_000_000);
    assert_eq!(report["loan_outstanding_after_day29_cents"], 0);
    assert_eq!(report["dividend_status"], "distributed");
    assert_eq!(report["dividend_total_cents"], 1_260);
    assert!(value_i64(&report, "government_dividend_receipt_cents") > 0);
    assert_eq!(
        report["fiscal_supplier_receipt_cents"],
        report["government_dividend_receipt_cents"]
    );
    assert_eq!(report["close_status"], "closed");
    assert_eq!(report["exit_phase"], "Closed");
    assert_eq!(report["capital_returned_cents"], 612_800);
    assert_eq!(report["liquidation_surplus_distributed_cents"], 0);
    assert_eq!(report["total_cash_final_cents"], 3_000_000);
    assert_eq!(report["self_financing_claim"], "not_asserted");
}

#[test]
fn household_cash_pressure_pays_accrued_wages_and_waits_for_unsold_inventory() {
    let temp = Temp::new();
    let report = scenario::run(
        &temp.world("cash-shortage"),
        scenario::Scenario::HouseholdCashShortage,
    )
    .unwrap();
    assert_eq!(report["sales_total"], 219);
    assert_eq!(report["production_total"], 220);
    assert_eq!(report["raw_remaining"], 60);
    assert_eq!(report["product_remaining"], 1);
    assert_eq!(report["seller_payroll_accrued_cents"], 22_400);
    assert_eq!(report["seller_payroll_paid_cents"], 22_400);
    assert_eq!(report["tax_assessed_cents"], 938);
    assert_eq!(report["tax_paid_cents"], 938);
    assert_eq!(report["tax_claim_count"], 22);
    assert_eq!(report["retained_earnings_after_day29_cents"], -3_767);
    assert_ne!(report["dividend_status"], "distributed");
    assert_eq!(report["dividend_total_cents"], 0);
    assert_eq!(report["household_dividend_receipt_cents"], 0);
    assert_eq!(report["government_dividend_receipt_cents"], 0);
    assert_eq!(report["household_final_cash_cents"], 62);
    assert_eq!(report["loan_outstanding_after_day29_cents"], 0);
    assert_ne!(report["close_status"], "closed");
    assert_eq!(report["exit_phase"], "Liquidating");
    assert_eq!(report["initial_total_cash_cents"], 2_800_000);
    assert_eq!(report["total_cash_final_cents"], 2_800_000);
    assert_eq!(report["self_financing_claim"], "not_asserted");
}
