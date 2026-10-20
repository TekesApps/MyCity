#[test]
fn full_64_payroll_20000_claims_and_eight_real_deliveries_match_independent_30day_oracle() {
    let (reference, report) = mycity_headless::scale_reference::run().unwrap();
    assert_eq!(report["wages_paid_total"], 118784);
    assert_eq!(report["claims_paid_total"], 20000);
    assert_eq!(report["eight_hour_gate"], "not_run");
    assert_eq!(
        report["field"],
        serde_json::to_value(
            reference
                .engine
                .background()
                .unwrap()
                .field()
                .unwrap()
                .report()
        )
        .unwrap()
    );

    println!("T13A_FULL_REFERENCE {report}");
}
