use super::*;

fn request() -> RequestIdentity {
    RequestIdentity {
        wire_protocol: "mycity-air-worker-v1".into(),
        strategy: "run36-air-f32-v1".into(),
        ticket: AirTicketIdentity {
            world: 1,
            branch: 2,
            job: 3,
            incarnation: 4,
            generation: 5,
            attempt: 1,
            base_revision: 0,
            field_version: 0,
            start: 10,
            end: 20,
            rules_sha256: [1; 32],
            source_sha256: [2; 32],
            input_sha256: [3; 32],
            checkpoint_sha256: [4; 32],
        },
    }
}

fn execution() -> CpuExecutionIdentity {
    CpuExecutionIdentity {
        pid: 7,
        process_instance: None,
        supervisor_owner_instance: Some(u128::MAX),
        allocation: CpuAllocationIdentity::DirectGrant {
            pool_boot: u128::MAX,
            grant_id: 9,
        },
    }
}

fn observation(request: RequestIdentity, execution: CpuExecutionIdentity, bytes: u64) -> CpuObservation {
    CpuObservation {
        version: OBSERVATION_VERSION,
        request,
        execution,
        stage: ObservedStage::Complete,
        candidate: CandidateEvidence::HostReadHashValidated {
            sha256: [5; 32],
            bytes,
        },
        measurements: CpuMeasurements::unavailable(),
    }
}

#[test]
fn zero_revision_and_field_version_are_valid_and_full_identity_is_retained() {
    let identity = request();
    identity.validate().unwrap();
    assert_eq!(identity.ticket.base_revision, 0);
    assert_eq!(identity.ticket.field_version, 0);
    assert_eq!(identity.ticket.checkpoint_sha256, [4; 32]);
    assert_eq!(identity.wire_protocol, "mycity-air-worker-v1");
    assert_eq!(identity.strategy, "run36-air-f32-v1");
}

#[test]
fn changing_any_request_or_ticket_field_rejects_the_stale_observation() {
    let original = request();
    let owner = execution();
    let mut attempt = AttemptObservation::new(original.clone(), owner).unwrap();
    attempt.record(observation(original.clone(), owner, 1)).unwrap();

    let mut changed = Vec::new();
    macro_rules! change_ticket {
        ($field:ident, $value:expr) => {{
            let mut request = original.clone();
            request.ticket.$field = $value;
            changed.push(request);
        }};
    }
    let mut protocol = original.clone();
    protocol.wire_protocol.push_str("-changed");
    changed.push(protocol);
    let mut strategy = original.clone();
    strategy.strategy.push_str("-changed");
    changed.push(strategy);
    change_ticket!(world, 11);
    change_ticket!(branch, 12);
    change_ticket!(job, 13);
    change_ticket!(incarnation, 14);
    change_ticket!(generation, 15);
    change_ticket!(attempt, 2);
    change_ticket!(base_revision, 16);
    change_ticket!(field_version, 17);
    change_ticket!(start, 11);
    change_ticket!(end, 21);
    change_ticket!(rules_sha256, [6; 32]);
    change_ticket!(source_sha256, [7; 32]);
    change_ticket!(input_sha256, [8; 32]);
    change_ticket!(checkpoint_sha256, [9; 32]);

    for changed_request in changed {
        assert_eq!(
            attempt.record(observation(changed_request, owner, 1)),
            Err(ObservationError::StaleAttemptOrProcess)
        );
    }
}

#[test]
fn full_width_pool_boot_round_trips_without_truncation_and_allocations_stay_distinct() {
    let direct = execution();
    let value = serde_json::to_value(&direct).unwrap();
    assert_eq!(
        value["allocation"]["pool_boot"],
        "340282366920938463463374607431768211455"
    );
    assert_eq!(
        value["supervisor_owner_instance"],
        "340282366920938463463374607431768211455"
    );
    let decoded: CpuExecutionIdentity = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, direct);

    let unpooled = CpuAllocationIdentity::Unpooled;
    let unknown = CpuAllocationIdentity::Unknown {
        reason: UnknownAllocationReason::UnconfirmedPoolOwner,
    };
    let broker = CpuAllocationIdentity::BrokerLease {
        pool_boot: Some(u128::MAX),
        generation: Some(u64::MAX),
        lease_id: Some(u64::MAX - 1),
    };
    assert_ne!(unpooled, unknown);
    assert_ne!(broker, CpuAllocationIdentity::DirectGrant {
        pool_boot: u128::MAX,
        grant_id: u64::MAX - 1,
    });
}

#[test]
fn decimal_u128_encoding_rejects_zero_leading_zero_and_more_than_39_digits() {
    for invalid in ["0", "01", "1000000000000000000000000000000000000000"] {
        let value = serde_json::json!({
            "pid": 7,
            "process_instance": invalid,
            "supervisor_owner_instance": "1",
            "allocation": {"kind":"unpooled"}
        });
        assert!(serde_json::from_value::<CpuExecutionIdentity>(value).is_err());
    }
}

#[test]
fn stale_process_is_rejected_even_when_request_and_pool_boot_match() {
    let identity = request();
    let current = execution();
    let mut attempt = AttemptObservation::new(identity.clone(), current).unwrap();
    attempt.record(observation(identity.clone(), current, 1)).unwrap();

    let mut stale = observation(identity, current, 1);
    stale.execution.pid += 1;
    assert_eq!(
        attempt.record(stale),
        Err(ObservationError::StaleAttemptOrProcess)
    );
}

#[test]
fn exactly_eight_mib_candidate_is_valid_but_over_limit_is_rejected() {
    let identity = request();
    let owner = execution();
    observation(identity.clone(), owner, MAX_CPU_CANDIDATE_BYTES)
        .validate()
        .unwrap();
    assert_eq!(
        observation(identity, owner, MAX_CPU_CANDIDATE_BYTES + 1).validate(),
        Err(ObservationError::InvalidCandidate)
    );
}

#[test]
fn worker_claim_and_host_read_validation_remain_distinct_evidence() {
    let identity = request();
    let owner = execution();
    let mut claimed = observation(identity.clone(), owner, 12);
    claimed.candidate = CandidateEvidence::WorkerDeclared {
        sha256: [5; 32],
        bytes: 12,
    };
    claimed.validate().unwrap();
    let host_checked = observation(identity, owner, 12);
    host_checked.validate().unwrap();
    assert_ne!(claimed.candidate, host_checked.candidate);
}

#[test]
fn unknown_measurements_are_not_zero_or_given_fake_intervals() {
    let unknown = Measurement::unknown(
        MeasurementMeaning::CompletedWork,
        MeasurementUnit::WorkUnits,
        UnknownMeasurementReason::NotInstrumented,
    );
    unknown.validate().unwrap();
    assert_eq!(unknown.value, MeasurementValue::Unknown {
        reason: UnknownMeasurementReason::NotInstrumented,
    });
    assert_eq!(unknown.source, MeasurementSource::Unavailable);
    assert_eq!(unknown.interval, None);
    assert_eq!(
        CpuMeasurements::unavailable().elapsed_time.meaning,
        MeasurementMeaning::HostElapsedFromSubmitToReceive
    );

    let measured_zero = Measurement {
        meaning: MeasurementMeaning::ProcessCpuTime,
        unit: MeasurementUnit::Nanoseconds,
        value: MeasurementValue::Known { value: 0 },
        source: MeasurementSource::HostProcessAccounting,
        interval: Some(MonotonicInterval { start_ns: 50, end_ns: 50 }),
    };
    measured_zero.validate().unwrap();
    assert_ne!(unknown.value, MeasurementValue::Known { value: 0 });

    let elapsed = Measurement {
        meaning: MeasurementMeaning::HostElapsedFromSubmitToReceive,
        unit: MeasurementUnit::Nanoseconds,
        value: MeasurementValue::Known { value: 40 },
        source: MeasurementSource::HostMonotonicClock,
        interval: Some(MonotonicInterval { start_ns: 10, end_ns: 50 }),
    };
    elapsed.validate().unwrap();
}

#[test]
fn measurement_slots_are_bound_and_work_or_scratch_cannot_be_fabricated() {
    let identity = request();
    let owner = execution();

    let mut swapped = observation(identity.clone(), owner, 1);
    swapped.measurements.cpu_time = swapped.measurements.elapsed_time;
    assert_eq!(swapped.validate(), Err(ObservationError::InvalidMeasurement));

    let mut swapped = observation(identity.clone(), owner, 1);
    swapped.measurements.work_units = swapped.measurements.scratch_peak;
    assert_eq!(swapped.validate(), Err(ObservationError::InvalidMeasurement));

    let wall_clock = Measurement {
        meaning: MeasurementMeaning::HostElapsedFromSubmitToReceive,
        unit: MeasurementUnit::Nanoseconds,
        value: MeasurementValue::Known { value: 40 },
        source: MeasurementSource::HostMonotonicClock,
        interval: Some(MonotonicInterval { start_ns: 10, end_ns: 50 }),
    };
    let mut wall_clock_as_work = observation(identity.clone(), owner, 1);
    wall_clock_as_work.measurements.work_units = Measurement {
        meaning: MeasurementMeaning::CompletedWork,
        unit: MeasurementUnit::WorkUnits,
        ..wall_clock
    };
    assert_eq!(
        wall_clock_as_work.validate(),
        Err(ObservationError::InvalidMeasurement)
    );

    let mut wall_clock_as_scratch = observation(identity, owner, 1);
    wall_clock_as_scratch.measurements.scratch_peak = Measurement {
        meaning: MeasurementMeaning::AttemptScratchPeak,
        unit: MeasurementUnit::Bytes,
        ..wall_clock
    };
    assert_eq!(
        wall_clock_as_scratch.validate(),
        Err(ObservationError::InvalidMeasurement)
    );
}

#[test]
fn application_instance_refines_once_without_rewriting_prior_unknown_history() {
    let identity = request(); let owner = execution();
    let mut attempt = AttemptObservation::new(identity.clone(), owner).unwrap();
    attempt.record(observation(identity.clone(), owner, 1)).unwrap();
    attempt.bind_process_instance(u128::MAX).unwrap();
    assert_eq!(attempt.latest().unwrap().execution.process_instance, None);
    assert!(attempt.bind_process_instance(0).is_err());
    assert!(attempt.bind_process_instance(2).is_err());
    assert!(attempt.record(observation(identity.clone(), owner, 1)).is_err());
    let mut bound = owner; bound.process_instance = Some(u128::MAX);
    attempt.record(observation(identity, bound, 1)).unwrap();
    assert_eq!(attempt.latest().unwrap().execution.process_instance, Some(u128::MAX));
}

#[test]
fn child_cpu_clock_is_distinct_from_host_elapsed_and_requires_exact_delta() {
    let mut metric = Measurement {
        meaning: MeasurementMeaning::ProcessCpuTime, unit: MeasurementUnit::Nanoseconds,
        value: MeasurementValue::Known { value: 20 }, source: MeasurementSource::ChildProcessAccounting,
        interval: Some(MonotonicInterval { start_ns: 100, end_ns: 120 }),
    };
    metric.validate().unwrap();
    metric.value = MeasurementValue::Known { value: 19 };
    assert_eq!(metric.validate(), Err(ObservationError::InvalidMeasurement));
    metric.value = MeasurementValue::Known { value: 20 }; metric.source = MeasurementSource::HostMonotonicClock;
    assert_eq!(metric.validate(), Err(ObservationError::InvalidMeasurement));
}
