use mycity_core::{
    town_economy::{exit::*, AccountSnapshot, InventorySnapshot},
    units::{Money, Quantity},
};

fn snapshot() -> ExitSnapshot {
    serde_json::from_str(include_str!("fixtures/town-business-exit/solvent.json")).unwrap()
}

fn liability(id: u64, class: ClaimClass, amount: i64, due_at: u64) -> ExitLiability {
    ExitLiability {
        source: LiabilitySource::TimelineClaim { id },
        class,
        payee: 700 + id as u32,
        due_at,
        outstanding: Money::new(amount).unwrap(),
    }
}

fn planned_settlements(plan: &ExitPlan) -> Vec<(LiabilitySource, i64)> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            ExitAction::SettleLiability { source, amount, .. } => Some((*source, amount.value())),
            _ => None,
        })
        .collect()
}

fn distributions(plan: &ExitPlan) -> Vec<(u32, i64)> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            ExitAction::DistributeResidual {
                shareholder,
                amount,
            } => Some((*shareholder, amount.value())),
            _ => None,
        })
        .collect()
}

#[test]
fn solvent_fixture_keeps_protected_cash_and_distributes_only_company_residual() {
    let input = snapshot();
    let unchanged = input.clone();
    let plan = plan_exit(&input).unwrap();
    assert_eq!(input, unchanged);
    assert_eq!(plan_exit(&input).unwrap(), plan);
    assert_eq!(plan.status, ExitStatus::ReadyToClose);
    assert_eq!(plan.cash_for_due_claims, Money::new(9_000).unwrap());
    assert_eq!(distributions(&plan), vec![(1001, 6_000), (1002, 3_000)]);
    assert!(plan
        .actions
        .iter()
        .any(|action| matches!(action, ExitAction::FinalizeCompanyExit { company: 50 })));
}

#[test]
fn due_wages_and_debts_follow_explicit_priority_without_owner_distribution() {
    let mut input = snapshot();
    input.company_account = AccountSnapshot {
        account: 500,
        cash: Money::new(100).unwrap(),
        protected: Money::ZERO,
    };
    input.priority_policy = Some(ExitPriorityPolicy {
        order: [
            ClaimClass::Wage,
            ClaimClass::Tax,
            ClaimClass::SecuredDebt,
            ClaimClass::UnsecuredDebt,
            ClaimClass::Other,
        ],
        within_tier: WithinTierRule::ProRataByOutstandingStableId,
    });
    input.liabilities = vec![
        liability(1, ClaimClass::Wage, 80, 10),
        liability(2, ClaimClass::SecuredDebt, 80, 10),
    ];
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.status, ExitStatus::ClaimsUnpaid);
    assert_eq!(
        planned_settlements(&plan),
        vec![
            (LiabilitySource::TimelineClaim { id: 1 }, 80),
            (LiabilitySource::TimelineClaim { id: 2 }, 20)
        ]
    );
    assert_eq!(plan.unpaid_due_liabilities, Money::new(60).unwrap());
    assert!(distributions(&plan).is_empty());
    assert!(plan
        .actions
        .iter()
        .any(|action| matches!(action, ExitAction::StopNewInputs { company: 50 })));
}

#[test]
fn same_tier_partial_payment_uses_pro_rata_integer_cents_and_stable_remainder() {
    let mut input = snapshot();
    input.company_account.cash = Money::new(2).unwrap();
    input.company_account.protected = Money::ZERO;
    input.liabilities = vec![
        liability(10, ClaimClass::Tax, 1, 10),
        liability(11, ClaimClass::Tax, 2, 10),
    ];
    let plan = plan_exit(&input).unwrap();
    assert_eq!(
        planned_settlements(&plan),
        vec![
            (LiabilitySource::TimelineClaim { id: 10 }, 1),
            (LiabilitySource::TimelineClaim { id: 11 }, 1),
        ]
    );
    assert_eq!(plan.unpaid_due_liabilities, Money::new(1).unwrap());
    assert!(distributions(&plan).is_empty());
}

#[test]
fn future_claims_block_distribution_even_when_no_due_claims_remain() {
    let mut input = snapshot();
    input.liabilities = vec![liability(1, ClaimClass::UnsecuredDebt, 500, 11)];
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.status, ExitStatus::WaitingForFutureClaims);
    assert_eq!(plan.future_liabilities, Money::new(500).unwrap());
    assert!(planned_settlements(&plan).is_empty());
    assert!(distributions(&plan).is_empty());
}

#[test]
fn inventory_offer_is_not_cash_and_requires_a_fresh_snapshot_to_distribute() {
    let mut input = snapshot();
    input.company_account.cash = Money::new(1_000).unwrap();
    input.company_account.protected = Money::new(1_000).unwrap();
    input.inventory = vec![ExitInventoryLot {
        inventory: InventorySnapshot {
            stock_id: 40,
            owner: 500,
            good: 0,
            quantity: Quantity::new(250).unwrap(),
            spendable_basis: Money::new(50).unwrap(),
            origin_quantity: Quantity::new(250).unwrap(),
            origin_basis: Money::new(50).unwrap(),
            offset: Quantity::ZERO,
        },
        sale_ready: true,
    }];
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.status, ExitStatus::WaitingForInventorySale);
    assert_eq!(plan.cash_for_due_claims, Money::ZERO);
    assert!(plan.actions.iter().any(|action| matches!(
        action,
        ExitAction::OfferExistingInventory { stock_id: 40, quantity } if quantity.value() == 250
    )));
    assert!(distributions(&plan).is_empty());
}

#[test]
fn missing_tax_or_legal_completeness_blocks_claims_and_distribution() {
    let mut input = snapshot();
    input.liabilities = vec![liability(1, ClaimClass::Wage, 100, 10)];
    input.tax_assessment_complete = false;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::IncompleteTaxAssessment));
    assert!(planned_settlements(&plan).is_empty());
    assert!(distributions(&plan).is_empty());

    input.tax_assessment_complete = true;
    input.asset_register_complete = false;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::IncompleteAssetRegister));
    assert!(planned_settlements(&plan).is_empty());

    input.asset_register_complete = true;
    input.legal_clearance = LegalClearance::Unknown;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::LegalClearanceUnknown));
    assert!(planned_settlements(&plan).is_empty());
}

#[test]
fn missing_share_register_does_not_block_due_creditor_payment_but_blocks_residual() {
    let mut input = snapshot();
    input.shareholders_complete = false;
    input.liabilities = vec![liability(1, ClaimClass::Wage, 100, 10)];
    input.company_account.cash = Money::new(200).unwrap();
    input.company_account.protected = Money::ZERO;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::MissingShareholderRegister));
    assert_eq!(
        planned_settlements(&plan),
        vec![(LiabilitySource::TimelineClaim { id: 1 }, 100)]
    );
    assert!(distributions(&plan).is_empty());
}

#[test]
fn incomplete_employment_terms_do_not_cancel_payroll() {
    let mut input = snapshot();
    input.payroll = vec![ExitPayroll {
        id: 7,
        active: true,
    }];
    input.employment_exit_terms_complete = false;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::IncompleteEmploymentTerms));
    assert!(plan
        .actions
        .iter()
        .any(|action| matches!(action, ExitAction::StopNewProduction { company: 50 })));
    assert!(!plan
        .actions
        .iter()
        .any(|action| matches!(action, ExitAction::CancelFuturePayroll { .. })));
}

#[test]
fn personal_inventory_cannot_be_counted_as_company_asset() {
    let mut input = snapshot();
    input.inventory = vec![ExitInventoryLot {
        inventory: InventorySnapshot {
            stock_id: 40,
            owner: 1001,
            good: 0,
            quantity: Quantity::new(1).unwrap(),
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::new(1).unwrap(),
            origin_basis: Money::ZERO,
            offset: Quantity::ZERO,
        },
        sale_ready: true,
    }];
    assert_eq!(plan_exit(&input), Err(ExitError::InvalidSnapshot));
}

#[test]
fn duplicate_liability_identity_and_invalid_lot_basis_are_rejected() {
    let mut input = snapshot();
    input.liabilities = vec![
        liability(1, ClaimClass::Wage, 10, 10),
        liability(1, ClaimClass::UnsecuredDebt, 10, 10),
    ];
    assert_eq!(plan_exit(&input), Err(ExitError::DuplicateIdentity));

    input.liabilities.clear();
    input.inventory = vec![ExitInventoryLot {
        inventory: InventorySnapshot {
            stock_id: 40,
            owner: 500,
            good: 0,
            quantity: Quantity::new(10).unwrap(),
            spendable_basis: Money::new(99).unwrap(),
            origin_quantity: Quantity::new(10).unwrap(),
            origin_basis: Money::new(100).unwrap(),
            offset: Quantity::ZERO,
        },
        sale_ready: true,
    }];
    assert_eq!(plan_exit(&input), Err(ExitError::InvalidSnapshot));
}

#[test]
fn unreviewed_priority_and_oversized_snapshots_fail_closed() {
    let mut input = snapshot();
    input.priority_policy = None;
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.blocked, Some(ExitBlock::MissingPriorityPolicy));
    input.priority_policy = Some(ExitPriorityPolicy {
        order: [
            ClaimClass::Tax,
            ClaimClass::Wage,
            ClaimClass::SecuredDebt,
            ClaimClass::UnsecuredDebt,
            ClaimClass::Other,
        ],
        within_tier: WithinTierRule::Unsupported,
    });
    let unsupported = plan_exit(&input).unwrap();
    assert_eq!(unsupported.blocked, Some(ExitBlock::MissingPriorityPolicy));
    input.inventory = (0..=MAX_EXIT_INVENTORY_LOTS)
        .map(|index| ExitInventoryLot {
            inventory: InventorySnapshot {
                stock_id: index as u32 + 1,
                owner: 500,
                good: 0,
                quantity: Quantity::ZERO,
                spendable_basis: Money::ZERO,
                origin_quantity: Quantity::ZERO,
                origin_basis: Money::ZERO,
                offset: Quantity::ZERO,
            },
            sale_ready: true,
        })
        .collect();
    assert_eq!(plan_exit(&input), Err(ExitError::Limit));
}

#[test]
fn fully_cleared_company_pays_claims_before_pro_rata_residual() {
    let mut input = snapshot();
    input.company_account.cash = Money::new(100).unwrap();
    input.company_account.protected = Money::new(10).unwrap();
    input.liabilities = vec![liability(2, ClaimClass::Tax, 20, 10)];
    let plan = plan_exit(&input).unwrap();
    assert_eq!(plan.status, ExitStatus::ReadyToClose);
    assert_eq!(plan.cash_after_claims, Money::new(70).unwrap());
    assert_eq!(
        planned_settlements(&plan),
        vec![(LiabilitySource::TimelineClaim { id: 2 }, 20)]
    );
    assert_eq!(distributions(&plan), vec![(1001, 47), (1002, 23)]);
    let settle_at = plan
        .actions
        .iter()
        .position(|action| matches!(action, ExitAction::SettleLiability { .. }))
        .unwrap();
    let distribute_at = plan
        .actions
        .iter()
        .position(|action| matches!(action, ExitAction::DistributeResidual { .. }))
        .unwrap();
    let finalize_at = plan
        .actions
        .iter()
        .position(|action| matches!(action, ExitAction::FinalizeCompanyExit { .. }))
        .unwrap();
    assert!(settle_at < distribute_at && distribute_at < finalize_at);
}

#[test]
fn maximum_bounded_snapshot_and_plan_heap_estimate_fit_the_budget() {
    let input_rows = MAX_EXIT_LIABILITIES * std::mem::size_of::<ExitLiability>()
        + MAX_EXIT_PAYROLLS * std::mem::size_of::<ExitPayroll>()
        + MAX_EXIT_INVENTORY_LOTS * std::mem::size_of::<ExitInventoryLot>()
        + MAX_EXIT_SHAREHOLDERS * std::mem::size_of::<Shareholder>()
        + std::mem::size_of::<ExitSnapshot>()
        + 4 * std::mem::size_of::<Vec<u8>>();
    let output_rows = MAX_EXIT_ACTIONS * std::mem::size_of::<ExitAction>();
    assert!(input_rows + output_rows <= EXIT_PLANNER_HEAP_BUDGET);
}
