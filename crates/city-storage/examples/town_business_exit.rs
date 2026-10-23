//! Small Store-backed liquidation example. The fixed route quote is a fixture substitute,
//! not a full road-network integration test.
use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{env, path::PathBuf};

const RULES: &str = "town-business-exit-cli-v1";

fn opening(waiting: bool) -> Result<Vec<u8>, String> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(err)?;
    let company_cash = if waiting { 101 } else { 0 };
    value["sources"].as_array_mut().ok_or("sources")?.extend([
        json!({"id": 6, "kind": "inventory_capital", "amount": if waiting { 0 } else { 20 }}),
        json!({"id": 7, "kind": "inventory_capital", "amount": 0}),
    ]);
    value["sources"][2]["amount"] = company_cash.into();
    for account in value["accounts"].as_array_mut().ok_or("accounts")? {
        if account["id"] == 3 {
            account["cash"] = company_cash.into();
        }
    }
    value["stocks"].as_array_mut().ok_or("stocks")?.extend([
        json!({"id": 2, "owner": 3, "commodity": "raw", "quantity": if waiting { 0 } else { 20 }, "basis": if waiting { 0 } else { 20 }, "source": 6}),
        json!({"id": 3, "owner": 3, "commodity": "product", "quantity": 0, "basis": 0, "source": 7}),
    ]);

    let claims = if waiting {
        json!([
            {"id": 1, "payee": 4, "class": "secured_debt"},
            {"id": 2, "payee": 4, "class": "secured_debt"}
        ])
    } else {
        json!([{"id": 1, "payee": 4, "class": "unsecured_debt"}])
    };
    let payroll = if waiting {
        json!([])
    } else {
        json!([{"id": 1, "payee": 2}])
    };
    let policy = json!({
        "policy_id": 1,
        "company_account": 3,
        "requester": 2,
        "executor": 1,
        "legal_clearance": "cleared",
        "priority_policy": {
            "order": ["tax", "wage", "secured_debt", "unsecured_debt", "other"],
            "within_tier": "pro_rata_by_outstanding_stable_id"
        },
        "claims_complete": true,
        "tax_assessment_complete": true,
        "asset_register_complete": true,
        "employment_exit_terms_complete": true,
        "claims": claims,
        "payroll": payroll
    });
    value["town"] = if waiting {
        json!({
            "schema_version": 1,
            "id": 1,
            "households": [{"id": 1, "account": 2, "households": 1, "need_good": 1, "need_per_household": 10, "reference_price_per_q": 102}],
            "employers": [{"account": 3, "worker_limit": 1}],
            "shops": [{"shop_id": 1, "account": 3, "stock_id": 3, "good": 1, "shelf_price_per_q": 100}],
            "recipes": [],
            "delivery": [{"quote_id": 1, "shop": 1, "household_group": 1, "good": 1, "reachable": true, "route_source": 77, "freight_account": 1, "utility_account": 1, "available_capacity": 100, "freight_per_q": 1, "utility_surcharge_per_q": 1}],
            "exit_policy": policy
        })
    } else {
        json!({
            "schema_version": 1,
            "id": 1,
            "households": [{"id": 1, "account": 2, "households": 1, "need_good": 1, "need_per_household": 10, "reference_price_per_q": 102}],
            "employers": [{"account": 3, "worker_limit": 1}],
            "shops": [{"shop_id": 1, "account": 3, "stock_id": 3, "good": 1, "shelf_price_per_q": 100}],
            "recipes": [{"recipe_id": 1, "employer": 3, "assigned_workers": 1, "input_stock_id": 2, "input_per_output": 1, "output_good": 1, "output_stock_id": 3, "output_per_worker_day": 20, "waste_per_output_q": 0, "available_utility_per_day": 1000, "utility_per_output_q": 1, "utility_cost_per_output_q": 1, "freight_cost_per_input_q": 0, "disposal_cost_per_input_q": 0, "utility_account": 1, "freight_account": 1, "disposal_account": 1}],
            "delivery": [{"quote_id": 1, "shop": 1, "household_group": 1, "good": 1, "reachable": true, "route_source": 77, "freight_account": 1, "utility_account": 1, "available_capacity": 100, "freight_per_q": 1, "utility_surcharge_per_q": 1}],
            "exit_policy": policy
        })
    };
    serde_json::to_vec(&value).map_err(err)
}

fn envelope(store: &Store, action: Action) -> Result<Envelope, String> {
    let view = store.engine().map_err(err)?.world().view();
    Ok(Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    })
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<(), String> {
    let request = envelope(store, action)?;
    let outcome = store.execute(caller, request).map_err(err)?;
    outcome.model.map_err(|error| format!("{error:?}"))?;
    if !outcome.has_persistent_receipt {
        return Err("missing_persistent_receipt".into());
    }
    Ok(())
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..256 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("missing_timeline")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline_not_complete:{target}"))
}

fn settle_market(store: &mut Store) -> Result<mycity_core::town_runtime::MarketSummary, String> {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 })?;
    Ok(store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("missing_town")?
        .state
        .last_summary)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: town_business_exit STORE_DIR [--waiting]")?;
    let waiting = match args.next().as_deref() {
        None => false,
        Some("--waiting") => true,
        Some(_) => return Err("usage: town_business_exit STORE_DIR [--waiting]".into()),
    };
    if args.next().is_some() {
        return Err("usage: town_business_exit STORE_DIR [--waiting]".into());
    }

    let mut store = Store::create(&dir, &opening(waiting)?, vec![], RULES).map_err(err)?;
    let initial_household_cash = cash(&store, 2)?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    if waiting {
        for (id, amount) in [(1, 100), (2, 100)] {
            execute(
                &mut store,
                Caller::System,
                Action::Timeline {
                    command: TimelineCommand::Claims {
                        first_id: id,
                        count: 1,
                        payer: 3,
                        payee: 4,
                        amount: Money::new(amount).map_err(err)?,
                        due: 2 * DAY,
                    },
                },
            )?;
        }
    } else {
        execute(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll {
                    spec: PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: Money::new(100).map_err(err)?,
                        cycle: Cycle::Daily,
                        ends_at: 4 * DAY,
                    },
                },
            },
        )?;
        execute(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Claims {
                    first_id: 1,
                    count: 1,
                    payer: 3,
                    payee: 4,
                    amount: Money::new(1_200).map_err(err)?,
                    due: 2 * DAY,
                },
            },
        )?;
    }

    advance_to(&mut store, DAY)?;
    if !waiting {
        let treasury = store
            .engine()
            .map_err(err)?
            .world()
            .account_ref(1)
            .map_err(err)?;
        let company = store
            .engine()
            .map_err(err)?
            .world()
            .account_ref(3)
            .map_err(err)?;
        execute(
            &mut store,
            Caller::Account(1),
            Action::Transfer {
                from: treasury,
                to: company,
                amount: Money::new(20).map_err(err)?,
            },
        )?;
    }
    let day1 = settle_market(&mut store)?;
    if !waiting
        && (day1.freight != Money::new(10).map_err(err)?
            || day1.utilities != Money::new(30).map_err(err)?)
    {
        return Err(format!(
            "day1_fee_mismatch:freight={}:utilities={}",
            day1.freight.value(),
            day1.utilities.value()
        ));
    }
    execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    )?;
    advance_to(&mut store, 2 * DAY)?;
    let day2 = settle_market(&mut store)?;
    execute(
        &mut store,
        Caller::Account(1),
        Action::SettleExit { company: 3 },
    )?;

    if waiting {
        let timeline = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("missing_timeline")?;
        let remaining = timeline
            .claims
            .iter()
            .map(|claim| claim.principal.value() - claim.paid.value())
            .sum::<i64>();
        let close_request = envelope(&store, Action::CloseExit { company: 3 })?;
        let close = store
            .execute(Caller::Account(1), close_request)
            .map_err(err)?;
        if close.model.is_ok() || !close.has_persistent_receipt {
            return Err("waiting_liquidation_unexpectedly_closed".into());
        }
        println!(
            "mode=waiting day1_sales={} day2_sales={} company_cash={} unpaid_claims={} phase=Liquidating close=blocked route_source=none full_road_network=not_tested",
            day1.actual_sales.value(),
            day2.actual_sales.value(),
            cash(&store, 3)?,
            remaining
        );
    } else {
        execute(
            &mut store,
            Caller::Account(1),
            Action::CloseExit { company: 3 },
        )?;
        let payroll = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("missing_timeline")?
            .payroll[0];
        let claim = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("missing_timeline")?
            .claims[0];
        let household_cash = cash(&store, 2)?;
        let expected_household_cash = initial_household_cash + payroll.paid.value()
            - (day1.actual_sales.value() + day2.actual_sales.value()) * 102;
        if household_cash != expected_household_cash {
            return Err(format!(
                "household_cash_mismatch:actual={household_cash}:expected={expected_household_cash}"
            ));
        }
        println!(
            "mode=closed startup_grant=20 self_funding=false day1_produced={} day1_sales={} day2_produced={} day2_sales={} company_cash={} household_cash={} wage_paid={} debt_paid={} raw_remaining={} product_remaining={} phase=Closed route_source=77 fixed_quote_substitute=true full_road_network=not_tested",
            day1.produced.value(),
            day1.actual_sales.value(),
            day2.produced.value(),
            day2.actual_sales.value(),
            cash(&store, 3)?,
            household_cash,
            payroll.paid.value(),
            claim.paid.value(),
            quantity(&store, 2)?,
            quantity(&store, 3)?
        );
    }
    let engine = store.engine().map_err(err)?;
    let project_bytes = std::mem::size_of::<mycity_core::construction::Project>();
    let profile_bytes = std::mem::size_of::<mycity_core::town_runtime::MarketProfile>();
    let state_bytes = std::mem::size_of::<mycity_core::town_runtime::MarketState>();
    let policy_bytes = std::mem::size_of::<mycity_core::town_runtime::ExitPolicy>();
    let engine_owned_bytes = engine.owned_capacity_bytes();
    println!(
        "memory_layout_bytes project={} project_bound={} engine_owned_capacity={} engine_bound={} market_profile={} market_state={} exit_policy={} process_peak=not_measured physical_memory_acceptance=not_measured full_16gb=not_run",
        project_bytes,
        mycity_core::construction::STATE_BUDGET_BYTES,
        engine_owned_bytes,
        mycity_core::transaction::ENGINE_BYTE_BUDGET,
        profile_bytes,
        state_bytes,
        policy_bytes
    );
    Ok(())
}

fn cash(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == id)
        .map(|account| account.cash.value())
        .ok_or_else(|| format!("missing_account:{id}"))
}

fn quantity(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == id)
        .map(|stock| stock.quantity.value())
        .ok_or_else(|| format!("missing_stock:{id}"))
}

fn err(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
