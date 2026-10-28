//! Payroll JobGroup member arbitration against the existing finite-city source fixture.
use mycity_core::{
    arena::Scope,
    assembly,
    boot::{Command as BootCommand, Facility},
    housing::{self, GenesisLeaseOffer},
    member_qualification::MemberRange,
    model_rows::Rows,
    payroll_members::{Binding, Opening as PayrollOpening, Profile, Source, LIMIT},
    service_delivery_authority::{Command as ServiceCommand, Motion},
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Engine, Envelope, TxError},
    units::Money,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../../../tests/fixtures/finite-city/opening.rs"]
mod fixture;

const PAYER: u32 = 7;
const PAYEE: u32 = 2;
const END: u64 = 3 * DAY;
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

fn housing_origin(input: &Value) -> [u8; 32] {
    let households = input["housing"]["households"].as_array().unwrap();
    let mut hash = Sha256::new();
    hash.update(b"venue-housing-genesis-members-v1\0household-id-ascending");
    hash.update(1_u64.to_le_bytes()); // world
    hash.update(1_u64.to_le_bytes()); // branch
    hash.update((households.len() as u64).to_le_bytes());
    for household in households {
        hash.update(
            u32::try_from(household["household_id"].as_u64().unwrap())
                .unwrap()
                .to_le_bytes(),
        );
        hash.update(
            u32::try_from(household["account"].as_u64().unwrap())
                .unwrap()
                .to_le_bytes(),
        );
        hash.update(
            u16::try_from(household["people"].as_u64().unwrap())
                .unwrap()
                .to_le_bytes(),
        );
        hash.update(household["source_id"].as_u64().unwrap().to_le_bytes());
    }
    hash.finalize().into()
}

fn binding(id: u32, start: u64, end: u64, hourly_cents: i64) -> Binding {
    Binding {
        spec: PayrollSpec {
            id,
            payer: PAYER,
            payee: PAYEE,
            home_city: 0,
            hourly_cents: Money::new(hourly_cents).unwrap(),
            cycle: Cycle::Daily,
            ends_at: END,
        },
        members: MemberRange { start, end },
        source: Source {
            id: 21_000 + u64::from(id),
            version: 1,
        },
    }
}

fn opening_input(bindings: &[Binding]) -> Value {
    let mut input = fixture::boot_service_input();
    let origin: [u8; 32] = serde_json::from_value(
        input["member_activity"]["population_origin"].clone(),
    )
    .unwrap();
    // Keep the fixture's original identity only after independently matching the
    // member ledger and Opening rule to the validated Housing household hash.
    assert_eq!(origin, housing_origin(&input));
    let mut rows = Rows::<Binding, LIMIT>::new().unwrap();
    for row in bindings {
        rows.try_push(*row).unwrap();
    }
    let opening = PayrollOpening {
        version: 1,
        profile: Profile::LocalDevelopmentActualJobGroupV1,
        scope: Scope::new(1, 1).unwrap(),
        population_origin: origin,
        member_version: 1,
        bindings: rows,
    };
    input["payroll_members"] = serde_json::to_value(opening).unwrap();
    input
}

fn engine(bindings: &[Binding]) -> (Engine, Value) {
    let input = opening_input(bindings);
    let bytes = serde_json::to_vec(&input).unwrap();
    let engine = Engine::new(assembly::assemble(&bytes).unwrap(), fixture::resources()).unwrap();
    (engine, input)
}

fn attempt(e: &mut Engine, caller: Caller, action: Action) -> Result<(), TxError> {
    let (scope, revision) = e.world().scope_revision();
    let operation_id = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
    e.execute(
        caller,
        Envelope {
            world: scope.world(),
            branch: scope.branch(),
            operation_id,
            expected_revision: revision,
            actions: vec![action],
        },
    )
    .map(|_| ())
}

fn apply(e: &mut Engine, caller: Caller, action: Action) {
    attempt(e, caller, action).unwrap();
}

fn activate(e: &mut Engine, input: &Value) {
    apply(
        e,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    let offer: GenesisLeaseOffer =
        serde_json::from_value(input["housing"]["lease_offers"][0].clone()).unwrap();
    apply(
        e,
        Caller::Account(PAYEE),
        Action::HousingLease {
            offer_id: offer.offer_id,
            offer_sha256: housing::genesis_lease_offer_sha256(offer),
        },
    );
}

fn add_payroll(e: &mut Engine, row: Binding) {
    apply(
        e,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll { spec: row.spec },
        },
    );
}

fn boot_service_inputs(e: &mut Engine) {
    for (facility, until) in [(Facility::Pump, 60), (Facility::Generator, 120)] {
        apply(
            e,
            Caller::System,
            Action::Boot {
                command: BootCommand::Start {
                    facility,
                    reverse_request: false,
                },
            },
        );
        advance(e, until);
    }
}

fn advance(e: &mut Engine, target: u64) {
    for _ in 0..4096 {
        let progress = e.timeline().unwrap().progress;
        if progress.target.is_none() && progress.last_complete == target {
            return;
        }
        apply(
            e,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance { target, limit: 256 },
            },
        );
    }
    panic!("timeline did not complete target {target}");
}

fn advance_until_p5_at(e: &mut Engine, target: u64) {
    for _ in 0..4096 {
        let progress = e.timeline().unwrap().progress;
        if progress.phase == Some(mycity_core::schedule::Phase::P5)
            && progress.at == target
            && progress.target == Some(target)
        {
            return;
        }
        apply(
            e,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance { target, limit: 256 },
            },
        );
    }
    panic!("timeline did not pause in P5 at {target}");
}

fn payroll(e: &Engine, id: u32) -> mycity_core::timeline::Payroll {
    *e.timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|row| row.spec.id == id)
        .unwrap()
}

#[test]
fn payroll21_member_2_earns_one_group_rate_for_a_normal_eight_hour_shift() {
    let group = binding(21, 2, 3, 10);
    let (mut e, input) = engine(&[group]);
    activate(&mut e, &input);
    add_payroll(&mut e, group);

    advance(&mut e, 17 * 3_600);

    let row = payroll(&e, 21);
    assert_eq!(row.work_seconds, 8 * 3_600);
    assert_eq!(row.accrued, Money::new(80).unwrap());
    assert_eq!(row.remainder, 0);
}

#[test]
fn two_member_job_group_still_uses_hourly_cents_as_the_whole_group_rate() {
    let group = binding(21, 2, 4, 10);
    let (mut e, input) = engine(&[group]);
    activate(&mut e, &input);
    add_payroll(&mut e, group);

    advance(&mut e, 17 * 3_600);

    let row = payroll(&e, 21);
    assert_eq!(row.work_seconds, 8 * 3_600);
    assert_eq!(row.accrued, Money::new(80).unwrap());
}

#[test]
fn service_occupancy_blocks_only_overlapping_payroll_and_release_starts_new_eligible_time() {
    let overlapping = binding(21, 0, 1, 10);
    let independent = binding(22, 2, 3, 10);
    let (mut e, input) = engine(&[overlapping, independent]);
    activate(&mut e, &input);
    add_payroll(&mut e, overlapping);
    add_payroll(&mut e, independent);
    boot_service_inputs(&mut e);

    advance(&mut e, 8 * 3_600);
    apply(
        &mut e,
        Caller::Account(3),
        Action::ServiceDelivery {
            command: ServiceCommand::Accept {
                household: 7,
                crew: 1,
            },
        },
    );
    // The public-service crew owns [0,2) from before 09:00 through 09:25.
    advance(&mut e, 9 * 3_600 + 25 * 60);
    assert_eq!(
        e.service_needs_summary()
            .unwrap()
            .delivery
            .unwrap()
            .trips[0]
            .motion,
        Motion::Finished
    );
    advance(&mut e, 17 * 3_600);

    let waited = payroll(&e, 21);
    assert_eq!(waited.work_seconds, 27_300);
    assert_eq!(waited.accrued, Money::new(75).unwrap());
    assert_eq!(waited.remainder, 3_000);
    let independent = payroll(&e, 22);
    assert_eq!(independent.work_seconds, 28_800);
    assert_eq!(independent.accrued, Money::new(80).unwrap());
}

#[test]
fn payroll_member_claim_blocks_public_service_until_stopwork_preserves_earned_wage() {
    let group = binding(21, 0, 1, 10);
    let (mut e, input) = engine(&[group]);
    activate(&mut e, &input);
    add_payroll(&mut e, group);
    boot_service_inputs(&mut e);
    advance(&mut e, 8 * 3_600);
    advance(&mut e, 9 * 3_600 + 10 * 60);

    let before_rejected_service = e.state_hash();
    assert!(attempt(
        &mut e,
        Caller::Account(3),
        Action::ServiceDelivery {
            command: ServiceCommand::Accept {
                household: 7,
                crew: 1,
            },
        },
    )
    .is_err());
    assert_eq!(e.state_hash(), before_rejected_service);

    let earned_before_stop = payroll(&e, 21);
    assert_eq!(earned_before_stop.work_seconds, 600);
    assert_eq!(earned_before_stop.accrued, Money::new(1).unwrap());
    assert_eq!(earned_before_stop.remainder, 2_400);
    apply(
        &mut e,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::StopWork { id: 21 },
        },
    );
    let stopped = payroll(&e, 21);
    assert!(!stopped.working);
    assert_eq!(stopped.work_seconds, 600);
    assert_eq!(stopped.accrued, Money::new(1).unwrap());
    assert_eq!(stopped.remainder, 2_400);
    assert_eq!(e.member_activity_summary().unwrap().active_count, 0);

    // Reuse the service request after StopWork commits the release. No full
    // outbound/onsite/return sequence is needed to prove the range was freed.
    apply(
        &mut e,
        Caller::Account(3),
        Action::ServiceDelivery {
            command: ServiceCommand::Accept {
                household: 7,
                crew: 1,
            },
        },
    );
    assert_eq!(e.member_activity_summary().unwrap().active_count, 1);
    assert_eq!(payroll(&e, 21).accrued, Money::new(1).unwrap());
}

#[test]
fn unregistered_or_mismatched_addpayroll_contracts_reject_atomically() {
    let group = binding(21, 2, 3, 10);
    let (mut e, input) = engine(&[group]);
    activate(&mut e, &input);

    let unregistered = PayrollSpec {
        id: 22,
        ..group.spec
    };
    let before_unregistered = e.state_hash();
    assert_eq!(
        attempt(
            &mut e,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll { spec: unregistered },
            },
        ),
        Err(TxError::InvalidOperation)
    );
    assert_eq!(e.state_hash(), before_unregistered);

    let wrong_amount = PayrollSpec {
        hourly_cents: Money::new(11).unwrap(),
        ..group.spec
    };
    let before_mismatch = e.state_hash();
    assert_eq!(
        attempt(
            &mut e,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll { spec: wrong_amount },
            },
        ),
        Err(TxError::InvalidOperation)
    );
    assert_eq!(e.state_hash(), before_mismatch);

    // The range is not supplied by AddPayroll; it is immutable Opening data.
    // A member source outside the Housing household range is rejected at opening.
    let out_of_range = binding(21, 10, 11, 10);
    let bytes = serde_json::to_vec(&opening_input(&[out_of_range])).unwrap();
    assert!(assembly::assemble(&bytes).is_err());

    add_payroll(&mut e, group);
    assert_eq!(e.timeline().unwrap().payroll.len(), 1);
    assert_eq!(payroll(&e, 21).spec, group.spec);
}

#[test]
fn stop_work_and_termination_preserve_unpaid_accrued_wage_debt() {
    // The finite fixture gives company 7 only 100,000 cents; one eight-hour
    // shift at this whole-group rate accrues 800,000 cents.
    let group = binding(21, 2, 4, 100_000);
    let (mut e, input) = engine(&[group]);
    activate(&mut e, &input);
    add_payroll(&mut e, group);
    advance(&mut e, 17 * 3_600);

    let earned = payroll(&e, 21);
    assert_eq!(earned.work_seconds, 28_800);
    assert_eq!(earned.accrued, Money::new(800_000).unwrap());
    assert_eq!(earned.paid, Money::ZERO);

    apply(
        &mut e,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::StopWork { id: 21 },
        },
    );
    let stopped = payroll(&e, 21);
    assert!(!stopped.working);
    assert_eq!(stopped.work_seconds, earned.work_seconds);
    assert_eq!(stopped.accrued, earned.accrued);

    apply(
        &mut e,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Terminate { id: 21 },
        },
    );
    let terminated = payroll(&e, 21);
    assert!(!terminated.working);
    assert_eq!(terminated.accrued, Money::new(800_000).unwrap());
    assert_eq!(terminated.billed, terminated.accrued);
    assert_eq!(terminated.paid, Money::new(100_000).unwrap());
    assert_eq!(
        terminated.accrued.checked_sub(terminated.paid).unwrap(),
        Money::new(700_000).unwrap()
    );
    advance(&mut e, DAY);
    let recovered = payroll(&e, 21);
    assert_eq!(recovered.work_seconds, 28_800);
    assert_eq!(recovered.accrued, Money::new(800_000).unwrap());
    assert_eq!(recovered.paid, Money::new(100_000).unwrap());
}

#[test]
fn two_opening_jobs_admitted_by_separate_envelopes_resume_the_same_p5_boundary() {
    let first = binding(21, 2, 3, 10);
    let second = binding(22, 3, 4, 10);
    let (mut e, input) = engine(&[first, second]);
    activate(&mut e, &input);

    // Each immutable registered job enters the original Timeline in its own
    // committed Envelope before the shift boundary is advanced.
    let before_first = e.world().scope_revision().1;
    add_payroll(&mut e, first);
    assert_eq!(e.world().scope_revision().1, before_first + 1);
    let before_second = e.world().scope_revision().1;
    add_payroll(&mut e, second);
    assert_eq!(e.world().scope_revision().1, before_second + 1);
    assert_eq!(e.timeline().unwrap().payroll.len(), 2);

    // At 09:00 P5, only one new member claim is admitted per Envelope. The
    // second registered job stays pending until the same boundary is resumed.
    advance_until_p5_at(&mut e, 9 * 3_600);
    assert_eq!(e.member_activity_summary().unwrap().active_count, 1);
    let checkpoint = e.export_checkpoint().unwrap();
    let checkpoint_hash = e.state_hash();

    // A raw checkpoint decode is intentionally unauthenticated. It may be
    // inspected for identity, but it cannot advance either payroll or service
    // state until the Store verifies the original admission/member receipts.
    let mut raw = Engine::restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(raw.state_hash(), checkpoint_hash);
    assert!(!raw.payroll_members_is_ready());
    assert!(!raw.service_needs_is_authenticated());
    let rejected = attempt(
        &mut raw,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Advance {
                target: 9 * 3_600,
                limit: 256,
            },
        },
    );
    assert_eq!(rejected, Err(TxError::InvalidOperation));
    assert_eq!(raw.state_hash(), checkpoint_hash);

    // Keep the original authenticated Engine and continue its pending P5 in a
    // new Envelope; Store-backed cold recovery is covered by its own test.
    assert_eq!(e.state_hash(), checkpoint_hash);
    assert!(e.payroll_members_is_ready());
    assert!(e.service_needs_is_authenticated());

    // Resume the pending P5 boundary in a new Envelope, then work one hour.
    advance(&mut e, 9 * 3_600);
    assert_eq!(e.member_activity_summary().unwrap().active_count, 2);
    advance(&mut e, 10 * 3_600);
    for id in [21, 22] {
        let row = payroll(&e, id);
        assert_eq!(row.work_seconds, 3_600);
        assert_eq!(row.accrued, Money::new(10).unwrap());
        assert_eq!(row.remainder, 0);
    }
}

#[test]
fn end_for_exit_releases_actual_members_but_keeps_wage_debt_and_fraction(){
    let group=binding(21,2,3,10);let(mut e,input)=engine(&[group]);activate(&mut e,&input);add_payroll(&mut e,group);
    advance(&mut e,9*3600+600);
    apply(&mut e,Caller::System,Action::Timeline{command:TimelineCommand::EndForExit{id:21}});
    let row=payroll(&e,21);assert_eq!((row.work_seconds,row.accrued.value(),row.billed.value(),row.paid.value(),row.remainder),(600,1,1,0,2400));
    assert!(!row.working);assert_eq!(row.next_due,None);assert_eq!(row.spec.ends_at,END);
    assert_eq!(e.member_activity_summary().unwrap().active_count,0);
    advance(&mut e,DAY);assert_eq!(payroll(&e,21).paid,Money::ZERO);
    apply(&mut e,Caller::System,Action::Timeline{command:TimelineCommand::SettlePayroll{id:21}});
    let row=payroll(&e,21);assert_eq!((row.work_seconds,row.accrued.value(),row.paid.value(),row.remainder),(600,1,1,2400));
}

#[test]
fn registered_contract_expiry_stops_future_claims_without_restarting_work(){
    let mut group=binding(21,2,3,10);group.spec.ends_at=DAY;
    let(mut e,input)=engine(&[group]);activate(&mut e,&input);add_payroll(&mut e,group);
    advance(&mut e,DAY);let first=payroll(&e,21);
    assert_eq!((first.work_seconds,first.accrued.value(),first.paid.value()),(28800,80,80));assert!(!first.working);assert_eq!(first.next_due,None);
    assert_eq!(e.member_activity_summary().unwrap().active_count,0);
    advance(&mut e,2*DAY);let second=payroll(&e,21);
    assert_eq!((second.work_seconds,second.accrued.value(),second.paid.value(),second.remainder),(28800,80,80,0));
    assert_eq!(e.member_activity_summary().unwrap().active_count,0);
}
