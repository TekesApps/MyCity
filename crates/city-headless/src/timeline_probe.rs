//! Actual finite dynamic source/ledger experiment. No UI, workers or GPU claims.
use mycity_core::{
    boot::{Command as B, Facility},
    exchange::Command as X,
    inbox::Command as I,
    timeline::{Command as T, Cycle, PayrollSpec},
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use mycity_storage::{Query, Store};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn money(n: i64) -> Money {
    Money::new(n).expect("constant cents")
}
fn execute(s: &mut Store, a: Action) -> Result<Envelope, String> {
    let e = Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().map_err(err)?.world().view().revision,
        actions: vec![a],
    };
    let outcome = s.execute(Caller::System, e.clone()).map_err(err)?;
    outcome
        .model
        .map_err(|error| format!("{error:?}: {:?}", e.actions))?;
    if !outcome.has_persistent_receipt {
        return Err("missing_persistent_receipt".into());
    }
    Ok(e)
}
fn command(s: &mut Store, c: T) -> Result<Envelope, String> {
    execute(s, Action::Timeline { command: c })
}
fn drive(s: &mut Store, target: u64) -> Result<u64, String> {
    let mut pages = 0;
    loop {
        command(s, T::Advance { target, limit: 256 })?;
        pages += 1;
        if s.engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline")?
            .progress
            .target
            .is_none()
        {
            return Ok(pages);
        }
        if pages > 10000 {
            return Err("bounded_probe_page_limit".into());
        }
    }
}
fn report(s: &mut Store) -> Result<Value, String> {
    let e = s.engine().map_err(err)?;
    let t = e.timeline().ok_or("timeline")?;
    let sums = t.claims.iter().try_fold((0i64, 0i64), |(sum, paid), c| {
        Ok::<_, String>((
            sum.checked_add(c.principal.value()).ok_or("overflow")?,
            paid.checked_add(c.paid.value()).ok_or("overflow")?,
        ))
    })?;
    let v = json!({"progress":t.progress,"claim_count":t.claims.len(),"claim_principal_cents":sums.0,"claim_paid_cents":sums.1,"payroll":t.payroll,"world":e.world().view(),"engine_owned_bytes":e.owned_capacity_bytes(),"timeline_owned_bytes":t.owned_bytes(),"state_sha256":s.state_hash().map_err(err)?,"sequence":s.committed_sequence(),"continuation":s.continuation_status().map_err(err)?,"checkpoint":s.checkpoint_status().map_err(err)?,"capacity":s.capacity_status().map_err(err)?});
    Ok(v)
}
pub fn run(root: &Path) -> Result<Value, String> {
    std::fs::create_dir(root).map_err(err)?;
    let started = Instant::now();
    let rules = env!("MYCITY_SOURCE_SHA256");
    let path = root.join("dense");
    let mut s = Store::create(
        &path,
        include_bytes!("../../../fixtures/m0-e0-opening.json"),
        vec![],
        rules,
    )
    .map_err(err)?;
    command(&mut s, T::Enable)?;
    command(
        &mut s,
        T::Claims {
            first_id: 1,
            count: 20000,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 3600,
        },
    )?;
    let retry = command(
        &mut s,
        T::Advance {
            target: 3600,
            limit: 256,
        },
    )?;
    let ticket = s.continuation_status().map_err(err)?;
    let hash = s.state_hash().map_err(err)?;
    s.checkpoint().map_err(err)?;
    drop(s);
    let mut s = Store::open(&path, rules).map_err(err)?;
    let r = s.execute(Caller::System, retry).map_err(err)?;
    r.model.map_err(err)?;
    if hash != s.state_hash().map_err(err)? || ticket != s.continuation_status().map_err(err)? {
        return Err("checkpoint_or_exact_retry_changed_progress".into());
    }
    let pages = 1 + drive(&mut s, 3600)?;
    s.checkpoint().map_err(err)?;
    let dense = report(&mut s)?;
    if dense["claim_paid_cents"] != 20000 {
        return Err("dense_payment_invariant".into());
    }
    drop(s);
    let path = root.join("economy");
    let mut s = Store::create(
        &path,
        include_bytes!("../../../fixtures/m0-economy-opening.json"),
        vec![],
        rules,
    )
    .map_err(err)?;
    command(&mut s, T::Enable)?;
    let economic_pages = drive(&mut s, 86400)?;
    s.checkpoint().map_err(err)?;
    let economy = report(&mut s)?;
    drop(s);
    let path = root.join("joint");
    // A separate household receives roster wages; do not relabel the existing company.
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-exchange-opening.json"))
            .map_err(err)?;
    opening["sources"]
        .as_array_mut()
        .ok_or("sources")?
        .push(json!({"id":19,"kind":"cash_capital","amount":0}));
    opening["accounts"]
        .as_array_mut()
        .ok_or("accounts")?
        .push(json!({"id":5,"role":"household","cash":0,"source":19}));
    let opening = serde_json::to_vec(&opening).map_err(err)?;
    let mut s = Store::create_spatial(
        &path,
        &opening,
        [
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ]
        .into_iter()
        .map(|(id, n)| Resource::new(id, n).expect("constant resource"))
        .collect(),
        rules,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .map_err(err)?;
    execute(&mut s, Action::Inbox { command: I::Enable })?;
    execute(
        &mut s,
        Action::Boot {
            command: B::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    )?;
    execute(
        &mut s,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    )?;
    execute(
        &mut s,
        Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 10,
            tolerance: 0,
        },
    )?;
    command(&mut s, T::Enable)?;
    command(
        &mut s,
        T::AddPayroll {
            spec: PayrollSpec {
                id: 1,
                payer: 3,
                payee: 5,
                home_city: 1,
                hourly_cents: money(7),
                cycle: Cycle::Daily,
                ends_at: 2 * 86400,
            },
        },
    )?;
    let joint_pages = drive(&mut s, 2 * 86400)?;
    s.checkpoint().map_err(err)?;
    let joint = report(&mut s)?;
    let h = s.state_hash().map_err(err)?;
    let query = s.query(Query::Timeline, None, 8).map_err(err)?;
    if h != s.state_hash().map_err(err)? {
        return Err("query_mutated_state".into());
    }
    if joint["payroll"][0]["accrued"] != 112
        || joint["world"]["region"]["interaction_time"] != 2 * 86400
    {
        return Err("joint_clock_labor_invariant".into());
    }
    Ok(
        json!({"schema":"mycity-m0-t05b-evidence-v1","source_sha256":rules,"lock_sha256":env!("MYCITY_LOCK_SHA256"),"wall_seconds":started.elapsed().as_secs_f64(),"dense_pages":pages,"economy_pages":economic_pages,"joint_pages":joint_pages,"dense":dense,"economy":economy,"joint":joint,"timeline_query_example":query,"worker_process_pool":"not_implemented","ui":"not_implemented","gpu":"not_run","memory_16gb_gate":"not_run","economic_balance":"not_verified"}),
    )
}
