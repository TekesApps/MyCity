//! Executable laboratory probes. These assert actual model results, not prerecorded outputs.
use crate::{
    assembly::assemble,
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::Money,
};
use serde_json::{json, Value};

pub fn transaction_probe(input: &[u8]) -> Result<Value, String> {
    let world = assemble(input)?;
    let family = world.account_ref(2)?;
    let company = world.account_ref(3)?;
    let mut engine = Engine::new(
        world,
        vec![
            Resource::new(1, 10).map_err(err)?,
            Resource::new(2, 10).map_err(err)?,
        ],
    )
    .map_err(err)?;
    let envelope = |engine: &Engine, id, actions| {
        let v = engine.world().view();
        Envelope {
            world: v.scope.world(),
            branch: v.scope.branch(),
            operation_id: id,
            expected_revision: v.revision,
            actions,
        }
    };
    let command = envelope(
        &engine,
        1,
        vec![Action::Protect {
            account: family,
            amount: Money::new(10000).map_err(err)?,
        }],
    );
    let protection = engine
        .execute(Caller::Account(2), command.clone())
        .map_err(err)?;
    let before = engine.state_hash();
    let cmd = envelope(
        &engine,
        2,
        vec![Action::Transfer {
            from: family,
            to: company,
            amount: Money::new(195000).map_err(err)?,
        }],
    );
    if engine.execute(Caller::Account(2), cmd) != Err(TxError::InsufficientFunds)
        || engine.state_hash() != before
    {
        return Err("protection_probe_failed".into());
    }
    let cmd = envelope(
        &engine,
        3,
        vec![Action::PayProtected {
            from: family,
            to: company,
            amount: Money::new(10000).map_err(err)?,
        }],
    );
    engine.execute(Caller::System, cmd).map_err(err)?;
    let cmd = envelope(
        &engine,
        4,
        vec![Action::ReserveCapacity {
            resource: 1,
            units: 8,
        }],
    );
    engine.execute(Caller::Account(2), cmd).map_err(err)?;
    let before = engine.state_hash();
    let cmd = envelope(
        &engine,
        5,
        vec![
            Action::ReserveCapacity {
                resource: 2,
                units: 8,
            },
            Action::ReserveCapacity {
                resource: 1,
                units: 8,
            },
        ],
    );
    if engine.execute(Caller::Account(3), cmd) != Err(TxError::Capacity)
        || engine.state_hash() != before
    {
        return Err("atomic_capacity_probe_failed".into());
    }
    let replay = engine.execute(Caller::Account(2), command).map_err(err)?;
    if replay != protection || engine.state_hash() != before {
        return Err("retry_probe_failed".into());
    }
    let world = engine.world().view();
    if world.accounts.iter().map(|a| a.cash.value()).sum::<i64>() != 3_000_000 {
        return Err("cash_conservation_probe_failed".into());
    }
    Ok(
        json!({"result":"volatile_transaction_probes_passed","durability":"volatile_only","world":world,"state_sha256":engine.state_hash(),"resources":engine.resources(),"journal":engine.journal(),"owned_capacity_bytes":engine.owned_capacity_bytes(),"scope":"protection_payment_atomic_capacity_and_retry_only","persistent_recovery":"not_run"}),
    )
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

pub fn schedule_probe(input: &crate::Bootstrap) -> Result<Value, String> {
    use crate::{
        arena::Scope,
        calendar::Date,
        schedule::{random_u64, Bookmark, Due, Phase, Schedule},
        units::GameTime,
    };
    let scope = Scope::new(input.world, input.branch).map_err(err)?;
    let at = GameTime::new(3600).map_err(err)?;
    let target = GameTime::new(3601).map_err(err)?;
    let sources = || {
        (1..=20000)
            .map(|source| Due {
                at,
                phase: Phase::P2,
                source,
            })
            .collect::<Vec<_>>()
    };
    let mut schedule = Schedule::new(scope, sources()).map_err(err)?;
    let mut ids = Vec::with_capacity(20000);
    let mut max_page = 0;
    loop {
        let page = schedule
            .next_page(target)
            .map_err(err)?
            .ok_or("unexpected_schedule_completion")?;
        max_page = max_page.max(page.events.len());
        ids.extend(page.events.iter().map(|e| e.source));
        let t = page.ticket;
        schedule.acknowledge(t).map_err(err)?;
        if ids.len() == 8192 {
            break;
        }
    }
    let bytes = serde_json::to_vec(&schedule.bookmark()).map_err(err)?;
    let bookmark: Bookmark = serde_json::from_slice(&bytes).map_err(err)?;
    let old = schedule
        .next_page(target)
        .map_err(err)?
        .ok_or("missing_second_page")?
        .ticket;
    let mut restored = Schedule::restore(scope, sources(), bookmark.clone()).map_err(err)?;
    if restored.acknowledge(old).is_ok() {
        return Err("old_schedule_ticket_accepted".into());
    }
    while let Some(p) = restored.next_page(target).map_err(err)? {
        max_page = max_page.max(p.events.len());
        ids.extend(p.events.iter().map(|e| e.source));
        let t = p.ticket;
        restored.acknowledge(t).map_err(err)?;
    }
    if ids != (1..=20000).collect::<Vec<_>>() || max_page != 8192 {
        return Err("schedule_paging_probe_failed".into());
    }
    let jan = Date::new(2001, 1, 31).map_err(str::to_owned)?;
    let feb = jan.next_monthly(31).map_err(str::to_owned)?;
    let march = feb.next_monthly(31).map_err(str::to_owned)?;
    Ok(
        json!({"result":"static_due_paging_and_bookmark_probe_passed","processed":ids.len(),"max_ready_page":max_page,"processed_sha256":crate::sha256(&serde_json::to_vec(&ids).map_err(err)?),"mid_boundary":bookmark,"final_boundary":restored.bookmark(),"source_capacity_bytes":restored.owned_source_capacity_bytes(),"rng_sample":random_u64(input.seed,1,1,30),"monthly_anchor31":[jan,feb,march],"bookmark_recovery":"in_memory_serde_roundtrip_only","persistent_recovery":"not_run","payroll_amounts_and_world_atomicity":"not_integrated"}),
    )
}
