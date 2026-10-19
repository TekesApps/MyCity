use mycity_core::{
    assembly::assemble,
    region::Error,
    transaction::{Action, Caller, Engine, Envelope, TxError},
    units::Money,
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-region-opening.json");
fn engine() -> Engine {
    Engine::new(assemble(INPUT).unwrap(), vec![]).unwrap()
}
fn cmd(e: &Engine, actions: Vec<Action>) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: e.world().view().revision,
        actions,
    }
}
fn apply(e: &mut Engine, actions: Vec<Action>) {
    let c = cmd(e, actions);
    e.execute(Caller::System, c).unwrap();
}
fn money(e: &Engine) -> i64 {
    e.world()
        .view()
        .accounts
        .iter()
        .map(|a| a.cash.value())
        .sum()
}
fn people(e: &Engine) -> u64 {
    let r = e.world().view().region.unwrap();
    r.cities
        .iter()
        .flatten()
        .map(|c| u64::from(c.population))
        .sum::<u64>()
        + r.transits
            .iter()
            .flatten()
            .map(|t| u64::from(t.actual))
            .sum::<u64>()
}
#[test]
fn remote_discovery_and_approximate_migration_preserve_actual_people() {
    let mut e = engine();
    let r = e.world().view().region.unwrap();
    let candidates = r.candidates(1).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|c| (c.city, c.distance_m))
            .collect::<Vec<_>>(),
        vec![(3, 2000), (2, 3000)]
    );
    let total = people(&e);
    let command = cmd(
        &e,
        vec![Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 2000,
            tolerance: 2,
        }],
    );
    let receipt = e.execute(Caller::System, command.clone()).unwrap();
    assert_eq!(receipt.migrated[0], Some(1998));
    let r = e.world().view().region.unwrap();
    assert_eq!(r.city(1).unwrap().population, 2002);
    assert_eq!(r.city(2).unwrap().population, 100);
    let t = r.transits.iter().flatten().next().unwrap();
    assert_eq!(
        (t.requested, t.actual, t.shortfall, t.arrives_at),
        (2000, 1998, 2, 20)
    );
    assert_eq!(people(&e), total);
    // Existing multi-hop flow already occupies the shared final corridor.
    let c = cmd(
        &e,
        vec![Action::RegionMigrate {
            from: 3,
            to: 2,
            requested: 3,
            tolerance: 0,
        }],
    );
    let before = e.state_hash();
    assert_eq!(
        e.execute(Caller::System, c),
        Err(TxError::Region(Error::NoRoute))
    );
    assert_eq!(e.state_hash(), before);
    assert_eq!(e.execute(Caller::System, command).unwrap(), receipt);
    apply(&mut e, vec![Action::RegionAdvance { seconds: 19 }]);
    assert_eq!(
        e.world().view().region.unwrap().city(2).unwrap().population,
        100
    );
    apply(&mut e, vec![Action::RegionAdvance { seconds: 1 }]);
    let r = e.world().view().region.unwrap();
    assert_eq!(r.city(2).unwrap().population, 2098);
    assert!(r.transits.iter().all(Option::is_none));
    assert_eq!(people(&e), total);
    let c = cmd(
        &e,
        vec![Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 1,
            tolerance: 1,
        }],
    );
    assert_eq!(
        e.execute(Caller::System, c),
        Err(TxError::Region(Error::Shortfall))
    );
}
#[test]
fn switching_does_not_catch_up_replay_or_replenish_and_sleep_cannot_erase_debt() {
    let mut e = engine();
    let total = money(&e);
    apply(&mut e, vec![Action::RegionAdvance { seconds: 30 }]);
    let r = e.world().view().region.unwrap();
    assert_eq!(r.interaction_time, 30);
    assert_eq!(r.city(1).unwrap().development_elapsed, 3153600000);
    assert_eq!(r.city(2).unwrap().development_elapsed, 30);
    assert_eq!(r.obligations[0].unwrap().arrears.value(), 600000);
    assert_eq!(r.obligations[0].unwrap().next_due, None);
    let cash = e.world().view().accounts;
    for id in [1, 2, 3, 1] {
        apply(&mut e, vec![Action::RegionActivate { city: id }]);
    }
    assert_eq!(e.world().view().accounts, cash);
    let r = e.world().view().region.unwrap();
    assert_eq!(r.interaction_time, 30);
    assert!(r.snapshots.iter().flatten().any(|s| s.applied_sequence > 0));
    apply(&mut e, vec![Action::RegionAdvance { seconds: 100 }]);
    assert_eq!(
        e.world().view().region.unwrap().obligations[0]
            .unwrap()
            .arrears
            .value(),
        600000
    );
    // A later, real funding source pays existing arrears, without renewing the contract.
    let from = e.world().account_ref(1).unwrap();
    let to = e.world().account_ref(3).unwrap();
    apply(
        &mut e,
        vec![
            Action::Transfer {
                from,
                to,
                amount: Money::new(100000).unwrap(),
            },
            Action::RegionSettleDebt { obligation: 1 },
        ],
    );
    assert_eq!(
        e.world().view().region.unwrap().obligations[0]
            .unwrap()
            .arrears
            .value(),
        500000
    );
    assert_eq!(money(&e), total);
}
#[test]
fn persisted_hook_failure_and_late_action_failure_roll_back_the_entire_region() {
    let mut e = engine();
    let before = e.state_hash();
    let c = cmd(
        &e,
        vec![
            Action::RegionMigrate {
                from: 1,
                to: 2,
                requested: 2000,
                tolerance: 2,
            },
            Action::RegionAdvance { seconds: 30 },
        ],
    );
    let failed = e.execute_recorded(Caller::System, c, |_, _| Err("disk_failure"));
    assert_eq!(failed, Err("disk_failure"));
    assert_eq!(e.state_hash(), before);
    assert!(e.journal().is_empty());
    let c = cmd(
        &e,
        vec![
            Action::RegionAdvance { seconds: 30 },
            Action::RegionActivate { city: 99 },
        ],
    );
    assert_eq!(
        e.execute(Caller::System, c),
        Err(TxError::Region(Error::City))
    );
    assert_eq!(e.state_hash(), before);
    let c = cmd(&e, vec![Action::RegionAdvance { seconds: 10 }]);
    assert_eq!(e.execute(Caller::Account(3), c), Err(TxError::Unauthorized));
    assert_eq!(e.state_hash(), before);
}
#[test]
fn due_order_and_cash_do_not_depend_on_advance_chunk_size() {
    let mut v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
    v["region"]["obligations"][0]["amount"] = serde_json::json!(40000);
    v["region"]["obligations"].as_array_mut().unwrap().push(serde_json::json!({"id":2,"payer":2,"payee":3,"period_seconds":7,"amount":20000,"next_due":7,"expires_at":35,"arrears":0}));
    let bytes = serde_json::to_vec(&v).unwrap();
    let mut a = Engine::new(assemble(&bytes).unwrap(), vec![]).unwrap();
    let mut b = Engine::new(assemble(&bytes).unwrap(), vec![]).unwrap();
    apply(&mut a, vec![Action::RegionAdvance { seconds: 35 }]);
    for _ in 0..35 {
        apply(&mut b, vec![Action::RegionAdvance { seconds: 1 }]);
    }
    assert_eq!(a.world().view().accounts, b.world().view().accounts);
    let ra = a.world().view().region.unwrap();
    let rb = b.world().view().region.unwrap();
    assert_eq!(ra.obligations, rb.obligations);
    assert_eq!(ra.cities, rb.cities);
    assert_eq!(money(&a), money(&b));
}
#[test]
fn malformed_sources_bounds_and_overflow_never_admit_partial_worlds() {
    let v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
    for (field, value) in [("willing", 5001), ("housing_capacity", 3999)] {
        let mut x = v.clone();
        x["region"]["cities"][0][field] = serde_json::json!(value);
        assert!(assemble(&serde_json::to_vec(&x).unwrap()).is_err());
    }
    let mut x = v.clone();
    x["region"]["obligations"][0]["payer"] = serde_json::json!(999);
    assert!(assemble(&serde_json::to_vec(&x).unwrap()).is_err());
    let mut x = v.clone();
    x["region"]["cities"][1]["development_elapsed"] = serde_json::json!(u64::MAX);
    let mut e = Engine::new(assemble(&serde_json::to_vec(&x).unwrap()).unwrap(), vec![]).unwrap();
    let before = e.state_hash();
    let c = cmd(&e, vec![Action::RegionAdvance { seconds: 1 }]);
    assert_eq!(
        e.execute(Caller::System, c),
        Err(TxError::Region(Error::Overflow))
    );
    assert_eq!(e.state_hash(), before);
    assert!(engine().owned_capacity_bytes() <= mycity_core::transaction::ENGINE_BYTE_BUDGET);
}
