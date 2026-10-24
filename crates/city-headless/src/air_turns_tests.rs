use super::*;

fn quota() -> Quota { Quota { source_limit: 1, cell_limit: 1, pages: 1 } }
fn work() -> WorkCounters {
    WorkCounters { sources: 2, cells: 3, integrated_cells: 100_000, segments: 1, pages: 1 }
}

#[test]
fn quota_edges_and_invalid_values() {
    assert!(quota().validate().is_ok());
    assert!(Quota { source_limit: 8192, cell_limit: 1024, pages: 16 }.validate().is_ok());
    for q in [
        Quota { source_limit: 0, ..quota() }, Quota { source_limit: 8193, ..quota() },
        Quota { cell_limit: 0, ..quota() }, Quota { cell_limit: 1025, ..quota() },
        Quota { pages: 0, ..quota() }, Quota { pages: 17, ..quota() },
    ] { assert_eq!(q.validate(), Err(Error::InvalidQuota)); }
}

#[test]
fn canonical_instances_and_u128_boundary() {
    for value in ["1", "340282366920938463463374607431768211455"] {
        assert_eq!(validate_process_instance(value), Ok(()));
    }
    for value in ["", "0", "01", "+1", "-1", " 1", "1\n", "１",
        "340282366920938463463374607431768211456", "1000000000000000000000000000000000000000"] {
        assert_eq!(validate_process_instance(value), Err(Error::InvalidInstance));
    }
    assert_eq!(ChildTurns::new("0".into(), quota()), Err(Error::InvalidInstance));
    assert_eq!(OwnerTurns::new("1".into(), Quota { pages: 0, ..quota() }), Err(Error::InvalidQuota));
}

#[test]
fn independent_counters_zero_and_each_overflow() {
    assert_eq!(WorkCounters::ZERO.checked_add(work()), Ok(work()));
    assert_eq!(work().checked_add(WorkCounters::ZERO), Ok(work()));
    let one = WorkCounters { sources: 1, cells: 1, integrated_cells: 1, segments: 1, pages: 1 };
    for max in [
        WorkCounters { sources: u64::MAX, ..WorkCounters::ZERO },
        WorkCounters { cells: u64::MAX, ..WorkCounters::ZERO },
        WorkCounters { integrated_cells: u64::MAX, ..WorkCounters::ZERO },
        WorkCounters { segments: u64::MAX, ..WorkCounters::ZERO },
        WorkCounters { pages: u64::MAX, ..WorkCounters::ZERO },
    ] { assert_eq!(max.checked_add(one), Err(Error::Overflow)); }
}

#[test]
fn two_actors_one_pending_boundary_and_terminal_page() {
    let mut child = ChildTurns::new("12".into(), quota()).unwrap();
    let mut owner = OwnerTurns::new("12".into(), quota()).unwrap();
    assert_eq!(child.turn(), 1);
    assert_eq!(owner.prepare_continue(quota()), Err(Error::WrongPhase));
    let first = child.reach_boundary(work()).unwrap();
    assert!(child.is_waiting());
    let saved = child.clone();
    assert_eq!(child.reach_boundary(work()), Err(Error::WrongPhase));
    assert_eq!(child.finish(work()), Err(Error::WrongPhase));
    assert_eq!(child, saved);
    owner.accept_boundary(&first).unwrap();
    let saved = owner.clone();
    assert_eq!(owner.accept_boundary(&first), Err(Error::WrongPhase));
    assert_eq!(owner.prepare_continue(Quota { pages: 0, ..quota() }), Err(Error::InvalidQuota));
    assert_eq!(owner, saved);
    let next = Quota { source_limit: 8192, cell_limit: 1024, pages: 16 };
    let continuation = owner.prepare_continue(next).unwrap();
    assert_eq!(owner.turn(), 2);
    let saved = owner.clone();
    assert_eq!(owner.prepare_continue(next), Err(Error::WrongPhase));
    assert_eq!(owner.accept_boundary(&first), Err(Error::TurnMismatch));
    assert_eq!(owner, saved);
    assert_eq!(child.accept_continue(&continuation), Ok(next));
    let saved = child.clone();
    assert_eq!(child.accept_continue(&continuation), Err(Error::TurnMismatch));
    assert_eq!(child, saved);
    assert_eq!(child.quota(), next);
    assert_eq!(owner.quota(), next);
    child.finish(work()).unwrap();
    owner.finish(work()).unwrap();
    assert!(child.is_finished() && owner.is_finished());
    assert_eq!(child.total(), work().checked_add(work()).unwrap());
    assert_eq!(child.total(), owner.total());
    let saved = child.clone();
    assert_eq!(child.finish(WorkCounters::ZERO), Err(Error::WrongPhase));
    assert_eq!(child.reach_boundary(WorkCounters::ZERO), Err(Error::WrongPhase));
    assert_eq!(child, saved);
}

#[test]
fn invalid_identity_or_quota_never_advances_either_actor() {
    let mut child = ChildTurns::new("7".into(), quota()).unwrap();
    let mut owner = OwnerTurns::new("7".into(), quota()).unwrap();
    let valid = child.reach_boundary(work()).unwrap();
    for (version, turn, instance, expected) in [
        (2, 1, "7", Error::VersionMismatch), (1, 0, "7", Error::TurnMismatch),
        (1, 2, "7", Error::TurnMismatch), (1, 1, "6", Error::InstanceMismatch),
        (1, 1, "07", Error::InvalidInstance),
    ] {
        let saved_owner = owner.clone();
        let saved_child = child.clone();
        let boundary = Boundary { plan_version: version, turn, process_instance: instance.into(), work: work() };
        assert_eq!(owner.accept_boundary(&boundary), Err(expected));
        let continuation = Continue { plan_version: version, turn, process_instance: instance.into(), quota: quota() };
        assert_eq!(child.accept_continue(&continuation), Err(expected));
        assert_eq!(owner, saved_owner);
        assert_eq!(child, saved_child);
    }
    let saved = child.clone();
    assert_eq!(child.accept_continue(&Continue { plan_version: 1, turn: 1,
        process_instance: "7".into(), quota: Quota { cell_limit: 0, ..quota() } }), Err(Error::InvalidQuota));
    assert_eq!(child, saved);
    owner.accept_boundary(&valid).unwrap();
    child.accept_continue(&owner.prepare_continue(quota()).unwrap()).unwrap();
    let second = child.reach_boundary(WorkCounters::ZERO).unwrap();
    let saved = child.clone();
    assert_eq!(child.accept_continue(&Continue { plan_version: 1, turn: 1,
        process_instance: "7".into(), quota: quota() }), Err(Error::TurnMismatch));
    assert_eq!(child, saved);
    owner.accept_boundary(&second).unwrap();
}

#[test]
fn counter_and_turn_overflows_are_atomic() {
    let mut child = ChildTurns::new("1".into(), quota()).unwrap();
    child.0.total.cells = u64::MAX;
    let saved = child.clone();
    assert_eq!(child.reach_boundary(work()), Err(Error::Overflow));
    assert_eq!(child.finish(work()), Err(Error::Overflow));
    assert_eq!(child, saved);
    child.0.turn = u64::MAX;
    let saved = child.clone();
    assert_eq!(child.reach_boundary(WorkCounters::ZERO), Err(Error::Overflow));
    assert_eq!(child, saved);
    child.finish(WorkCounters::ZERO).unwrap();
    assert_eq!(child.total().cells, u64::MAX);
}

#[test]
fn reporting_has_no_cost_or_resource_authority() {
    // A report may include full-grid integration, or zero work. Acceptance
    // validates control only; neither report proves cost, execution or shortage.
    let mut child = ChildTurns::new("1".into(), quota()).unwrap();
    let boundary = child.reach_boundary(work()).unwrap();
    assert!(boundary.work.integrated_cells > u64::from(child.quota().cell_limit));
    let mut noop = ChildTurns::new("2".into(), quota()).unwrap();
    noop.finish(WorkCounters::ZERO).unwrap();
    assert_eq!(noop.total(), WorkCounters::ZERO);
    assert_eq!(PLAN_VERSION, 1);
    assert_eq!(PLAN_ID, "air-exact-pages-v1");
}

#[test]
fn dto_roundtrips_and_rejects_unknown_fields() {
    let q = quota();
    assert_eq!(serde_json::from_str::<Quota>(&serde_json::to_string(&q).unwrap()).unwrap(), q);
    let w = work();
    assert_eq!(serde_json::from_str::<WorkCounters>(&serde_json::to_string(&w).unwrap()).unwrap(), w);
    let b = Boundary { plan_version: 1, turn: 1, process_instance: "1".into(), work: w };
    assert_eq!(serde_json::from_str::<Boundary>(&serde_json::to_string(&b).unwrap()).unwrap(), b);
    let c = Continue { plan_version: 1, turn: 1, process_instance: "1".into(), quota: q };
    assert_eq!(serde_json::from_str::<Continue>(&serde_json::to_string(&c).unwrap()).unwrap(), c);
    for mut value in [serde_json::to_value(q).unwrap(), serde_json::to_value(w).unwrap(),
        serde_json::to_value(b).unwrap(), serde_json::to_value(c).unwrap()] {
        value.as_object_mut().unwrap().insert("unexpected".into(), serde_json::Value::Null);
        assert!(serde_json::from_value::<Quota>(value.clone()).is_err());
        assert!(serde_json::from_value::<WorkCounters>(value.clone()).is_err());
        assert!(serde_json::from_value::<Boundary>(value.clone()).is_err());
        assert!(serde_json::from_value::<Continue>(value).is_err());
    }
}
