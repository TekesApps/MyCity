#[path = "../examples/support/household_wage_spatial.rs"]
mod scenario;

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn wage_sensitive_market_uses_verified_fixed_od_and_returns_tax_dividend_to_fiscal_spend() {
    let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join(format!(
        "mycity-household-wage-spatial-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&artifact).unwrap();
    let artifact = fs::canonicalize(artifact).unwrap();
    eprintln!("HOUSEHOLD_WAGE_SPATIAL_ARTIFACT={}", artifact.display());

    let report = scenario::run_pair(&artifact);
    assert_eq!(report.control.opening_receipt_sequence, 0);
    assert_eq!(report.higher_wage.opening_receipt_sequence, 0);
    for case in [&report.control, &report.higher_wage] {
        assert!(case.opening_store_owned_bytes > 0);
        assert!(case.opening_engine_owned_bytes > 0);
        assert!(case.opening_world_owned_bytes > 0);
        assert!(case.final_cold_receipt_sequence > 0);
        assert!(case.final_cold_store_owned_bytes > 0);
        assert!(case.final_cold_engine_owned_bytes > 0);
        assert!(case.final_cold_world_owned_bytes > 0);
        assert_eq!(case.final_cold_state_hash, case.state_hash);
    }
    assert_eq!(report.control.sales_by_day, [7, 3]);
    assert_eq!(report.higher_wage.sales_by_day, [10, 10]);
    assert_eq!(report.control.household_cash_after_day1, 93);
    assert_eq!(report.higher_wage.household_cash_after_day1, 30);
    assert_eq!(report.higher_wage.household_cash_final, 137);
    assert_eq!(report.control.taxable_basis_by_day, [63, 0]);
    assert_eq!(report.higher_wage.taxable_basis_by_day, [90, 90]);
    assert_eq!(report.control.tax_assessed_by_day, [31, 0]);
    assert_eq!(report.higher_wage.tax_assessed_by_day, [45, 45]);
    assert!(report.control.retained_earnings_before_distribution <= 0);
    assert_eq!(
        report.control.rejected_dividend_reason.as_deref(),
        Some("InvalidOperation")
    );
    assert_eq!(report.higher_wage.retained_earnings_before_distribution, 90);
    assert_eq!(report.higher_wage.rejected_dividend_reason, None);
    assert_eq!(report.higher_wage.seller_payroll_paid, 1_600);
    assert_eq!(report.higher_wage.supplemental_payroll_paid, 480);
    assert_eq!(report.higher_wage.household_dividend, 77);
    assert_eq!(report.higher_wage.government_dividend, 13);
    assert_eq!(report.higher_wage.dividend_total, 90);
    assert_eq!(
        report.higher_wage.government_dividend,
        report.higher_wage.supplier_fiscal_receipt
    );
    assert_eq!(report.higher_wage.household_cash_final, 137);
    assert_eq!(report.control.served_people_by_day, [[4, 4]; 3]);
    assert_eq!(report.higher_wage.served_people_by_day, [[4, 4]; 3]);
    assert_eq!(report.control.total_cash, 2_805_000);
    assert_eq!(report.higher_wage.total_cash, 2_805_000);

    eprintln!(
        "HOUSEHOLD_WAGE_SPATIAL_RESULT={}",
        scenario::report_json(&report)
    );
    if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_none() {
        let _ = fs::remove_dir_all(&artifact);
    } else {
        eprintln!("HOUSEHOLD_WAGE_SPATIAL_PRESERVED={}", artifact.display());
    }
}
