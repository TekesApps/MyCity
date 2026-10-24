//! Pure shape/provenance tests only. These fixtures are not process, grant,
//! wait, or Store authority and do not launch a worker.
use super::*;

fn request() -> Request {
    Request {
        protocol: PROTOCOL.into(),
        strategy: mycity_core::air::STRATEGY.into(),
        controlled: None,
        ticket: super::super::Ticket {
            world: 101,
            branch: 202,
            job: 303,
            incarnation: 404,
            generation: 2,
            attempt: 2,
            base_revision: 0,
            field_version: 0,
            start: 15,
            end: 28,
            rules_sha256: "1".repeat(64),
            source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
            input_sha256: "2".repeat(64),
            checkpoint_sha256: "3".repeat(64),
        },
    }
}

#[test]
fn request_projection_retains_every_original_wire_identity_field() {
    let request = request();
    let identity = request_identity(&request).unwrap();
    assert_eq!(identity.wire_protocol, request.protocol);
    assert_eq!(identity.strategy, request.strategy);
    assert_eq!(identity.ticket.world, request.ticket.world);
    assert_eq!(identity.ticket.branch, request.ticket.branch);
    assert_eq!(identity.ticket.job, request.ticket.job);
    assert_eq!(identity.ticket.incarnation, request.ticket.incarnation);
    assert_eq!(identity.ticket.generation, request.ticket.generation);
    assert_eq!(identity.ticket.attempt, request.ticket.attempt);
    assert_eq!(identity.ticket.base_revision, 0);
    assert_eq!(identity.ticket.field_version, 0);
    assert_eq!(identity.ticket.start, request.ticket.start);
    assert_eq!(identity.ticket.end, request.ticket.end);
    assert_eq!(identity.ticket.rules_sha256, [0x11; 32]);
    assert_eq!(identity.ticket.source_sha256, digest(&request.ticket.source_sha256).unwrap());
    assert_eq!(identity.ticket.input_sha256, [0x22; 32]);
    assert_eq!(identity.ticket.checkpoint_sha256, [0x33; 32]);
}

#[test]
fn current_candidate_is_host_read_validated_but_not_store_accepted() {
    let request = request();
    let event = Event {
        protocol: PROTOCOL.into(),
        ticket: request.ticket.clone(),
        pid: 909,
        stage: Stage::Complete,
        candidate_sha256: Some("a".repeat(64)),
        candidate_bytes: 8 * 1024 * 1024,
        cursor: None,
        cpu: None,
        turn: None,
        page_metrics: None,
    };
    let candidate = candidate_evidence(&event).unwrap();
    assert_eq!(candidate, schema::CandidateEvidence::HostReadHashValidated {
        sha256: [0xaa; 32],
        bytes: 8 * 1024 * 1024,
    });
    assert!(matches!(candidate, schema::CandidateEvidence::HostReadHashValidated { .. }));
    // The schema has no Store receipt or acceptance bit; Complete remains an observation.
}

#[test]
fn progress_cursor_is_a_worker_claim_not_measured_work() {
    let event = Event {
        protocol: PROTOCOL.into(),
        ticket: request().ticket,
        pid: 909,
        stage: Stage::Cells,
        candidate_sha256: None,
        candidate_bytes: 0,
        cursor: Some(8192),
        cpu: None,
        turn: None,
        page_metrics: None,
    };
    assert_eq!(
        candidate_evidence(&event).unwrap(),
        schema::CandidateEvidence::None { cursor_claim: 8192 }
    );
    let metrics = schema::CpuMeasurements::unavailable();
    assert_eq!(metrics.work_units.value, schema::MeasurementValue::Unknown {
        reason: schema::UnknownMeasurementReason::NotInstrumented,
    });
    assert_eq!(metrics.work_units.unit, schema::MeasurementUnit::WorkUnits);
    assert_eq!(metrics.work_units.interval, None);
}

#[test]
fn malformed_or_oversized_candidate_claim_is_rejected_by_core_schema() {
    let request = request_identity(&request()).unwrap();
    let execution = schema::CpuExecutionIdentity {
        pid: 909,
        process_instance: None,
        supervisor_owner_instance: Some(u128::MAX),
        allocation: schema::CpuAllocationIdentity::Unknown {
            reason: schema::UnknownAllocationReason::UnconfirmedPoolOwner,
        },
    };
    let event = Event {
        protocol: PROTOCOL.into(),
        ticket: request_to_wire(&request),
        pid: 909,
        stage: Stage::Complete,
        candidate_sha256: Some("a".repeat(64)),
        candidate_bytes: 8 * 1024 * 1024 + 1,
        cursor: None,
        cpu: None,
        turn: None,
        page_metrics: None,
    };
    let observation = schema::CpuObservation {
        version: schema::OBSERVATION_VERSION,
        request,
        execution,
        stage: schema::ObservedStage::Complete,
        candidate: candidate_evidence(&event).unwrap(),
        measurements: schema::CpuMeasurements::unavailable(),
    };
    assert_eq!(observation.validate(), Err(schema::ObservationError::InvalidCandidate));
}

fn request_to_wire(identity: &schema::RequestIdentity) -> super::super::Ticket {
    let hex = |digest: &[u8; 32]| {
        digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>()
    };
    super::super::Ticket {
        world: identity.ticket.world,
        branch: identity.ticket.branch,
        job: identity.ticket.job,
        incarnation: identity.ticket.incarnation,
        generation: identity.ticket.generation,
        attempt: identity.ticket.attempt,
        base_revision: identity.ticket.base_revision,
        field_version: identity.ticket.field_version,
        start: identity.ticket.start,
        end: identity.ticket.end,
        rules_sha256: hex(&identity.ticket.rules_sha256),
        source_sha256: hex(&identity.ticket.source_sha256),
        input_sha256: hex(&identity.ticket.input_sha256),
        checkpoint_sha256: hex(&identity.ticket.checkpoint_sha256),
    }
}

fn metric_attempt() -> Attempt {
    let request = request();
    let owner = schema::CpuExecutionIdentity { pid: 909, process_instance: Some(11),
        supervisor_owner_instance: Some(12), allocation: schema::CpuAllocationIdentity::Unpooled };
    Attempt { state: schema::AttemptObservation::new(request_identity(&request).unwrap(), owner).unwrap(),
        request, sent_at: std::time::Instant::now(), cpu_interval: None, invalid_cpu: false }
}
fn cpu_event(start_ns: u64, end_ns: u64) -> Event {
    Event { protocol: PROTOCOL.into(), ticket: request().ticket, pid: 909, stage: Stage::Cells,
        candidate_sha256: None, candidate_bytes: 0, cursor: Some(1),
        turn: None,
        page_metrics: None,
        cpu: Some(super::super::cpu_metrics::CpuTelemetry {
            metrics_version: 1, process_instance: Some("11".into()),
            source: super::super::cpu_metrics::MetricSource::ChildProcessAccounting,
            process_cpu_time: super::super::cpu_metrics::CpuTimeValue::Known {
                cumulative: super::super::cpu_metrics::KnownCpuTime { start_ns, end_ns, delta_ns: end_ns-start_ns },
            },
        }),
    }
}
#[test]
fn parent_rejects_instance_replacement_start_changes_and_regression_without_forging_zero() {
    for change in 0..4 {
        let mut attempt = metric_attempt();
        assert!(matches!(attempt.cpu_measurement(&cpu_event(100,120), Some(11)).value, schema::MeasurementValue::Known { value: 20 }));
        let mut event = cpu_event(100,130);
        match change {
            0 => event.cpu.as_mut().unwrap().process_instance = Some("13".into()),
            1 => event = cpu_event(101,130),
            2 => event = cpu_event(100,119),
            _ => event.cpu.as_mut().unwrap().metrics_version = 99,
        }
        assert!(matches!(attempt.cpu_measurement(&event, Some(11)).value, schema::MeasurementValue::Unknown { .. }));
        // Invalid continuity cannot silently heal with a newer well-shaped number.
        assert!(matches!(attempt.cpu_measurement(&cpu_event(100,150), Some(11)).value, schema::MeasurementValue::Unknown { .. }));
    }
    let mut attempt = metric_attempt(); let mut event = cpu_event(100,120); event.cpu = None;
    assert!(matches!(attempt.cpu_measurement(&event, Some(11)).value, schema::MeasurementValue::Unknown { .. }));
    assert!(matches!(attempt.cpu_measurement(&cpu_event(100,130), Some(11)).value, schema::MeasurementValue::Known { value: 30 }));
}
