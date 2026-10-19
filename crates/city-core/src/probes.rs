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
