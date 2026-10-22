use mycity_core::construction::{
    AbortClearanceMaterialRule, AbortClearanceSpec, Command, Effect, Error, Gate, GateCategory,
    GateDecision, Kind, Material, Phase, Project, Receipt, MAX_MATERIALS,
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
    let mut project = Project::new(
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
    .unwrap();
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 12;
    project.waste_receiver_resource = 82;
    project.waste_units = 3;
    project.waste_fee = Money::new(100).unwrap();
    project
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
    let publish = p
        .apply(
            at,
            Command::AcceptInspection {
                inspector: 3,
                accepted: true,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        publish.kind,
        mycity_core::construction::EffectKind::PublishOperatingCapacity
    );
    p.confirm(receipt(publish, false)).unwrap();
    assert_eq!(p.phase, Phase::Operational);
    assert!(p.operating_published);
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

fn clearance_project() -> Project {
    let mut p = project();
    p.abort_clearance_spec = Some(AbortClearanceSpec {
        base_worker_seconds: 3_600,
        material_rules: [
            AbortClearanceMaterialRule {
                stock_id: 9,
                quantity_per_waste_capacity_unit: 2_000,
                worker_seconds_per_thousand_quantity: 600,
            },
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
        ],
    });
    p
}

fn begin_clearance_review(p: &mut Project, at: GameTime) {
    p.apply(
        at,
        Command::BeginDemolition {
            waste_receiver: 12,
            gates: [
                g(3, 10),
                g(4, 11),
                Gate::default(),
                Gate::default(),
                Gate::default(),
                Gate::default(),
                Gate::default(),
                Gate::default(),
            ],
            gate_count: 2,
        },
    )
    .unwrap();
}

fn prepare_current_gates(p: &mut Project, at: GameTime) {
    for i in 0..usize::from(p.gate_count) {
        let gate_id = p.gates[i].id;
        p.apply(
            at,
            Command::DecideGate {
                gate_id,
                decision: GateDecision::Accepted,
                evidence_id: u64::from(gate_id),
            },
        )
        .unwrap();
    }
    p.apply(at, Command::Prepare).unwrap();
}

fn reserve_clearance_site(p: &mut Project) {
    approve(p);
    let at = GameTime::new(9 * 3_600).unwrap();
    let reserve = p
        .apply(at, Command::Start)
        .unwrap()
        .expect("construction reserves its site");
    p.confirm(receipt(reserve, false)).unwrap();
    // In Engine this token is issued atomically with ReserveSite.
    p.site_token = Some(77);
}

#[test]
fn unfinished_clearance_freezes_real_spend_and_consumption_then_releases_site_once() {
    let mut p = clearance_project();
    reserve_clearance_site(&mut p);
    let work_at = GameTime::new(17 * 3_600).unwrap();
    let work = p
        .propose_work(
            work_at,
            28_800,
            Money::new(100_000).unwrap(),
            [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
            14_400,
        )
        .unwrap()
        .unwrap();
    p.confirm(receipt(work, false)).unwrap();
    assert_eq!(p.materials[0].consumed, Quantity::new(500).unwrap());
    let original_spent = p.spent;
    p.apply(work_at, Command::Pause).unwrap();

    begin_clearance_review(&mut p, work_at);
    assert!(!p.can_request_uncommissioned_clearance());
    assert_eq!(p.apply(work_at, Command::Prepare), Err(Error::GatePending));
    p.apply(
        work_at,
        Command::DecideGate {
            gate_id: 3,
            decision: GateDecision::Rejected,
            evidence_id: 0,
        },
    )
    .unwrap();
    assert_eq!(p.phase, Phase::Paused);
    assert_eq!(p.waste_receiver, None);
    assert_eq!(p.site_token, Some(77));

    begin_clearance_review(&mut p, work_at);
    prepare_current_gates(&mut p, work_at);
    let original_materials = p.materials;
    assert_eq!(p.apply(work_at, Command::Start), Ok(None));
    assert!(p.is_uncommissioned_clearance());
    assert_eq!(p.kind, Kind::Demolish);
    assert_eq!(p.phase, Phase::Demolition);
    assert_eq!(p.site_token, Some(77));
    assert!(p.site_reserved);
    assert_eq!(p.material_count, 1);
    assert_eq!(p.materials, original_materials);
    assert_eq!(p.active_material_count(), 0);
    assert_eq!(p.planned_worker_seconds, 3_900);
    assert_eq!(p.effective_waste_units(), 1);
    assert_eq!(p.effective_waste_fee(), Money::new(100).unwrap());
    assert_eq!(p.validate_persisted(), Ok(()));

    let partial_at = GameTime::new(86_400 + 10 * 3_600).unwrap();
    let partial_elapsed = (partial_at.value() - work_at.value()) as u32;
    let partial = p
        .propose_work(
            partial_at,
            partial_elapsed,
            Money::new(100_000).unwrap(),
            [Quantity::ZERO; MAX_MATERIALS],
            1_900,
        )
        .unwrap()
        .unwrap();
    p.confirm(receipt(partial, false)).unwrap();
    assert_eq!(p.completed_worker_seconds, 1_900);

    let finish_at = GameTime::new(86_400 + 13 * 3_600).unwrap();
    let elapsed = (finish_at.value() - partial_at.value()) as u32;
    let clearance_work = p
        .propose_work(
            finish_at,
            elapsed,
            Money::new(100_000).unwrap(),
            [Quantity::ZERO; MAX_MATERIALS],
            2_000,
        )
        .unwrap()
        .unwrap();
    assert_eq!(clearance_work.materials, [Quantity::ZERO; MAX_MATERIALS]);
    let failed = Receipt {
        operation_id: clearance_work.operation_id,
        success: false,
        cash: clearance_work.cash,
        materials: clearance_work.materials,
        total_basis: clearance_work.total_basis,
        site_units: clearance_work.site_units,
        waste_accepted: false,
    };
    assert_eq!(p.confirm(failed), Err(Error::FailedEffect));
    assert_eq!(p.completed_worker_seconds, 1_900);
    let partial_spent = p.spent;
    assert!(partial_spent > original_spent);
    assert_eq!(p.materials, original_materials);
    let retry = p
        .propose_work(
            finish_at,
            elapsed,
            Money::new(100_000).unwrap(),
            [Quantity::ZERO; MAX_MATERIALS],
            2_000,
        )
        .unwrap()
        .unwrap();
    p.confirm(receipt(retry, false)).unwrap();
    assert_eq!(p.materials, original_materials);
    assert!(p.spent > partial_spent);
    assert_eq!(p.validate_persisted(), Ok(()));

    let disposal = p.apply(finish_at, Command::AcceptWaste).unwrap().unwrap();
    assert_eq!(disposal.cash, Money::new(100).unwrap());
    assert_eq!(disposal.waste_units, 1);
    p.confirm(receipt(disposal, true)).unwrap();
    let spent_after_receipt = p.spent;
    assert_eq!(
        p.apply(finish_at, Command::AcceptWaste),
        Err(Error::WrongPhase)
    );
    assert_eq!(p.spent, spent_after_receipt);
    assert_eq!(p.pending, None);
    let release = p.apply(finish_at, Command::Close).unwrap().unwrap();
    assert_eq!(release.site_units, 1);
    p.confirm(receipt(release, false)).unwrap();
    assert_eq!(p.phase, Phase::Closed);
    assert_eq!(p.site_token, None);
    assert!(!p.site_reserved);
    assert_eq!(p.validate_persisted(), Ok(()));
}

#[test]
fn zero_consumption_clearance_uses_base_work_without_fabricating_waste_or_fee() {
    let mut p = clearance_project();
    reserve_clearance_site(&mut p);
    let at = GameTime::new(9 * 3_600).unwrap();
    begin_clearance_review(&mut p, at);
    prepare_current_gates(&mut p, at);
    assert_eq!(p.apply(at, Command::Start), Ok(None));
    assert_eq!(p.planned_worker_seconds, 3_600);
    assert_eq!(p.effective_waste_units(), 0);
    assert_eq!(p.effective_waste_fee(), Money::ZERO);
    let finish = GameTime::new(17 * 3_600).unwrap();
    let e = p
        .propose_work(
            finish,
            28_800,
            Money::new(100_000).unwrap(),
            [Quantity::ZERO; MAX_MATERIALS],
            3_600,
        )
        .unwrap()
        .unwrap();
    assert_eq!(e.materials, [Quantity::ZERO; MAX_MATERIALS]);
    p.confirm(receipt(e, false)).unwrap();
    assert_eq!(p.apply(finish, Command::AcceptWaste), Ok(None));
    assert!(p.waste_accepted);
    assert_eq!(
        p.apply(finish, Command::AcceptWaste),
        Err(Error::WrongPhase)
    );
    assert_eq!(p.pending, None);
    assert_eq!(p.spent, Money::new(3_600).unwrap());
    let release = p.apply(finish, Command::Close).unwrap().unwrap();
    assert_eq!(release.site_units, 1);
    p.confirm(receipt(release, false)).unwrap();
    assert_eq!(p.site_token, None);
    assert_eq!(p.validate_persisted(), Ok(()));
}

#[test]
fn clearance_requires_a_valid_opening_spec_and_state_remains_within_budget() {
    let at = GameTime::new(9 * 3_600).unwrap();
    for spec in [
        None,
        Some(AbortClearanceSpec {
            base_worker_seconds: 3_600,
            material_rules: [AbortClearanceMaterialRule::default(); MAX_MATERIALS],
        }),
        Some(AbortClearanceSpec {
            base_worker_seconds: 0,
            material_rules: [
                AbortClearanceMaterialRule {
                    stock_id: 9,
                    quantity_per_waste_capacity_unit: 2_000,
                    worker_seconds_per_thousand_quantity: 600,
                },
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
            ],
        }),
    ] {
        let mut p = project();
        p.abort_clearance_spec = spec;
        reserve_clearance_site(&mut p);
        assert!(!p.can_request_uncommissioned_clearance());
        assert_eq!(
            p.apply(
                at,
                Command::BeginDemolition {
                    waste_receiver: 12,
                    gates: [
                        g(3, 10),
                        g(4, 11),
                        Gate::default(),
                        Gate::default(),
                        Gate::default(),
                        Gate::default(),
                        Gate::default(),
                        Gate::default()
                    ],
                    gate_count: 2,
                }
            ),
            Err(Error::InvalidInput)
        );
        assert_eq!(p.phase, Phase::Building);
        assert_eq!(p.site_token, Some(77));
        assert_eq!(p.pending, None);
    }
    assert!(
        std::mem::size_of::<Project>() <= mycity_core::construction::STATE_BUDGET_BYTES,
        "clearance metadata exceeds the fixed project state budget"
    );
}
