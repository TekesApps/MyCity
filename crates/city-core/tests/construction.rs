use mycity_core::construction::{
    Command, Effect, Error, Gate, GateCategory, GateDecision, Kind, Material, Phase, Project,
    Receipt, MAX_MATERIALS,
};
use mycity_core::units::{GameTime, Money, Quantity};

fn g(id: u32, party: u32) -> Gate {
    Gate {
        id,
        category: GateCategory::OwnerConsent,
        evidence_id: 0,
        party,
        decided: GateDecision::Pending,
    }
}
fn project() -> Project {
    Project::new(
        7,
        Kind::Build,
        1,
        2,
        3,
        80,
        1,
        2,
        Money::new(100_000).unwrap(),
        Money::new(3_600).unwrap(),
        28_800,
        &[g(1, 10), g(2, 11)],
        &[Material {
            stock_id: 9,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(5_000).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::new(9 * 3_600).unwrap(),
    )
    .unwrap()
}
fn receipt(e: Effect, waste: bool) -> Receipt {
    Receipt {
        operation_id: e.operation_id,
        success: true,
        cash: e.cash,
        materials: e.materials,
        total_basis: e.total_basis,
        site_units: e.site_units,
        waste_accepted: waste,
    }
}
fn approve(p: &mut Project) {
    p.apply(GameTime::new(9 * 3_600).unwrap(), Command::File)
        .unwrap();
    for (id, party) in [(1, 10), (2, 11)] {
        p.apply(
            GameTime::new(9 * 3_600).unwrap(),
            Command::DecideGate {
                gate_id: id,
                decision: GateDecision::Accepted,
                evidence_id: u64::from(id),
            },
        )
        .unwrap();
        assert!(party > 0);
    }
    p.apply(GameTime::new(9 * 3_600).unwrap(), Command::Prepare)
        .unwrap();
}

#[test]
fn consent_materials_wages_and_inspection_gate_occupancy() {
    let mut p = project();
    p.apply(GameTime::new(9 * 3_600).unwrap(), Command::File)
        .unwrap();
    assert_eq!(
        p.apply(GameTime::new(9 * 3_600).unwrap(), Command::Prepare),
        Err(Error::GatePending)
    );
    p.apply(
        GameTime::new(9 * 3_600).unwrap(),
        Command::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 1,
        },
    )
    .unwrap();
    p.apply(
        GameTime::new(9 * 3_600).unwrap(),
        Command::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 2,
        },
    )
    .unwrap();
    p.apply(GameTime::new(9 * 3_600).unwrap(), Command::Prepare)
        .unwrap();
    let e = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    assert_eq!(e.site_units, 1);
    p.confirm(receipt(e, false)).unwrap();
    assert_eq!(p.phase, Phase::Building);
    let at = GameTime::new(17 * 3_600).unwrap();
    assert_eq!(
        p.propose_work(
            at,
            28_800,
            Money::new(100_000).unwrap(),
            [Quantity::new(999).unwrap(); MAX_MATERIALS],
            28_800
        ),
        Err(Error::Materials)
    );
    let e = p
        .propose_work(
            at,
            28_800,
            Money::new(100_000).unwrap(),
            [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
            28_800,
        )
        .unwrap()
        .unwrap();
    assert_eq!(e.cash, Money::new(28_800).unwrap());
    assert_eq!(
        p.completed_worker_seconds, 0,
        "intent alone must not move durable progress"
    );
    assert_eq!(p.apply(at, Command::Pause), Err(Error::EffectPending));
    assert_eq!(p.confirm(receipt(e, false)), Ok(()));
    assert_eq!(p.completed_worker_seconds, 28_800);
    assert_eq!(p.spent, Money::new(33_800).unwrap());
    let e = p.apply(at, Command::SubmitInspection).unwrap().unwrap();
    p.confirm(receipt(e, false)).unwrap();
    assert_eq!(p.phase, Phase::Inspection);
    assert_eq!(
        p.apply(
            at,
            Command::AcceptInspection {
                inspector: 99,
                accepted: true
            }
        ),
        Err(Error::WrongPhase)
    );
    p.apply(
        at,
        Command::AcceptInspection {
            inspector: 3,
            accepted: true,
        },
    )
    .unwrap();
    assert_eq!(p.phase, Phase::Operational);
}

#[test]
fn work_is_rejected_when_cash_or_total_project_budget_is_insufficient() {
    let mut p = project();
    approve(&mut p);
    let reserve = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    p.confirm(receipt(reserve, false)).unwrap();
    let at = GameTime::new(17 * 3_600).unwrap();
    let stock = [Quantity::new(1_000).unwrap(); MAX_MATERIALS];
    assert_eq!(
        p.propose_work(at, 28_800, Money::new(28_799).unwrap(), stock, 28_800),
        Err(Error::Funds)
    );
    p.budget = Money::new(33_799).unwrap();
    assert_eq!(
        p.propose_work(at, 28_800, Money::new(100_000).unwrap(), stock, 28_800),
        Err(Error::Funds)
    );
    assert_eq!(p.pending, None);
    assert_eq!(p.completed_worker_seconds, 0);
}

#[test]
fn wall_speed_does_not_change_game_calendar_work_and_weekend_is_not_workday() {
    let mut p = project();
    p.crew = 1;
    p.planned_worker_seconds = 60 * 3_600;
    p.budget = Money::new(1_000_000).unwrap();
    approve(&mut p);
    let reserve = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    p.confirm(receipt(reserve, false)).unwrap();
    // Epoch day 0 is Monday. Through Saturday 17:00 there are only five 8-hour shifts.
    let saturday = GameTime::new(5 * 86_400 + 17 * 3_600).unwrap();
    assert_eq!(
        p.propose_work(
            saturday,
            (5 * 86_400 + 8 * 3_600) as u32,
            Money::new(1_000_000).unwrap(),
            [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
            40 * 3_600 + 1
        ),
        Err(Error::WorkLimit)
    );
    assert_eq!(p.completed_worker_seconds, 0);
}

#[test]
fn identical_effect_replay_is_stable_and_definitive_failure_does_not_charge_twice() {
    let mut p = project();
    approve(&mut p);
    let e = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    assert_eq!(p.pending, Some(e));
    let failure = Receipt {
        operation_id: e.operation_id,
        success: false,
        cash: e.cash,
        materials: e.materials,
        total_basis: e.total_basis,
        site_units: e.site_units,
        waste_accepted: false,
    };
    assert_eq!(p.confirm(failure), Err(Error::FailedEffect));
    assert!(!p.site_reserved);
    assert_eq!(p.phase, Phase::Ready);
    assert_eq!(p.failure_count, 1);
    let retry = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    assert_ne!(e.operation_id, retry.operation_id);
    p.confirm(receipt(retry, false)).unwrap();
    assert!(p.site_reserved);
}

#[test]
fn cannot_activate_uninspected_or_demolish_without_parties_and_waste_receiver() {
    let mut p = project();
    approve(&mut p);
    let reserve = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    p.confirm(receipt(reserve, false)).unwrap();
    assert_eq!(
        p.apply(
            GameTime::new(9 * 3_600).unwrap(),
            Command::AcceptInspection {
                inspector: 3,
                accepted: true
            }
        ),
        Err(Error::WrongPhase)
    );
    assert_eq!(
        p.apply(
            GameTime::new(9 * 3_600).unwrap(),
            Command::BeginDemolition {
                waste_receiver: 0,
                gates: [Gate::default(); 8],
                gate_count: 0
            }
        ),
        Err(Error::WrongPhase)
    );
}

#[test]
fn pending_effect_survives_serialization_and_cannot_be_acked_twice() {
    let mut p = project();
    approve(&mut p);
    let e = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    let bytes = serde_json::to_vec(&p).unwrap();
    assert!(
        std::mem::size_of::<Project>() <= mycity_core::construction::STATE_BUDGET_BYTES,
        "fixed in-memory project state exceeds the profile budget"
    );
    let mut restored: Project = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.pending, Some(e));
    assert_eq!(restored.confirm(receipt(e, false)), Ok(()));
    assert_eq!(
        restored.confirm(receipt(e, false)),
        Err(Error::ReceiptMismatch)
    );
    assert!(restored.site_reserved);
}

#[test]
fn cancellation_never_releases_started_capacity_or_refunds_spend() {
    let mut p = project();
    approve(&mut p);
    let e = p
        .apply(GameTime::new(9 * 3_600).unwrap(), Command::Start)
        .unwrap()
        .unwrap();
    p.confirm(receipt(e, false)).unwrap();
    assert_eq!(
        p.apply(GameTime::new(9 * 3_600).unwrap(), Command::Cancel),
        Err(Error::WrongPhase)
    );
    assert!(p.site_reserved);
    assert_eq!(p.spent, Money::ZERO);
}
