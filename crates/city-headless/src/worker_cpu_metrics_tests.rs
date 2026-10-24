use super::*;

#[test]
fn timespec_conversion_rejects_invalid_fields_and_checked_overflow() {
    assert_eq!(timespec_to_ns(1, 2), Ok(1_000_000_002));
    assert_eq!(
        timespec_to_ns(-1, 0),
        Err(UnknownReason::InvalidTimespec)
    );
    assert_eq!(
        timespec_to_ns(1, -1),
        Err(UnknownReason::InvalidTimespec)
    );
    assert_eq!(
        timespec_to_ns(1, 1_000_000_000),
        Err(UnknownReason::InvalidTimespec)
    );
    assert_eq!(
        timespec_to_ns(i64::MAX, 999_999_999),
        Err(UnknownReason::NanosecondOverflow)
    );
}

#[test]
fn initial_clock_failure_stays_unknown_without_zero_fallback() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(
        u128::MAX,
        Err(UnknownReason::ClockReadFailed),
    );
    let sample = sampler.sample_with_reading(Ok(20));
    assert_eq!(sample.process_instance.as_deref(), Some("340282366920938463463374607431768211455"));
    assert_eq!(
        sample.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::ClockReadFailed,
            last_known_lower_bound: None,
        }
    );
    sample.validate().unwrap();
}

#[test]
fn middle_failure_retains_only_last_known_lower_bound_and_can_recover() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(11, Ok(100));
    let first = sampler.sample_with_reading(Ok(120));
    assert_eq!(
        first.process_cpu_time,
        CpuTimeValue::Known {
            cumulative: KnownCpuTime { start_ns: 100, end_ns: 120, delta_ns: 20 },
        }
    );

    let failed = sampler.sample_with_reading(Err(UnknownReason::ClockReadFailed));
    assert_eq!(
        failed.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::ClockReadFailed,
            last_known_lower_bound: Some(KnownCpuTime {
                start_ns: 100,
                end_ns: 120,
                delta_ns: 20,
            }),
        }
    );

    let recovered = sampler.sample_with_reading(Ok(150));
    assert_eq!(
        recovered.process_cpu_time,
        CpuTimeValue::Known {
            cumulative: KnownCpuTime { start_ns: 100, end_ns: 150, delta_ns: 50 },
        }
    );
}

#[test]
fn clock_regression_latches_unknown_for_the_rest_of_the_attempt() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(12, Ok(100));
    let _ = sampler.sample_with_reading(Ok(150));
    let regressed = sampler.sample_with_reading(Ok(140));
    assert_eq!(
        regressed.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::ClockWentBackwards,
            last_known_lower_bound: None,
        }
    );
    let after_regression = sampler.sample_with_reading(Ok(200));
    assert_eq!(
        after_regression.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::ClockWentBackwards,
            last_known_lower_bound: None,
        }
    );
}

#[test]
fn clock_overflow_latches_unknown_for_the_rest_of_the_attempt() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(14, Ok(100));
    let overflow = sampler.sample_with_reading(Err(UnknownReason::NanosecondOverflow));
    assert_eq!(
        overflow.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::NanosecondOverflow,
            last_known_lower_bound: None,
        }
    );
    let later = sampler.sample_with_reading(Ok(200));
    assert_eq!(later.process_cpu_time, overflow.process_cpu_time);
}

#[test]
fn samples_are_cumulative_from_one_start_not_summed_event_deltas() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(13, Ok(1_000));
    let a = sampler.sample_with_reading(Ok(1_010));
    let b = sampler.sample_with_reading(Ok(1_025));
    assert_eq!(known(&a).delta_ns, 10);
    assert_eq!(known(&b).delta_ns, 25);
    assert_eq!(known(&b).start_ns, 1_000);
}

#[test]
fn each_request_gets_a_fresh_start_sample() {
    let mut first_request = CpuAttemptSampler::begin_with_sample(21, Ok(100));
    let first = first_request.sample_with_reading(Ok(160));
    assert_eq!(known(&first).delta_ns, 60);

    let mut second_request = CpuAttemptSampler::begin_with_sample(21, Ok(900));
    let second = second_request.sample_with_reading(Ok(925));
    assert_eq!(known(&second), KnownCpuTime { start_ns: 900, end_ns: 925, delta_ns: 25 });
}

#[test]
fn known_value_construction_is_only_a_schema_claim_not_clock_evidence() {
    let forged = CpuTelemetry {
        metrics_version: METRICS_VERSION,
        process_instance: Some("1".to_owned()),
        source: MetricSource::ChildProcessAccounting,
        process_cpu_time: CpuTimeValue::Known {
            cumulative: KnownCpuTime { start_ns: 5, end_ns: 5, delta_ns: 0 },
        },
    };
    // The DTO is structurally valid; only CpuAttemptSampler's private native
    // clock path can produce a runtime observation. Schema validity is not proof.
    forged.validate().unwrap();
    assert_eq!(
        serde_json::from_str::<CpuTelemetry>(&serde_json::to_string(&forged).unwrap()).unwrap(),
        forged
    );
}

#[test]
fn process_instance_decimal_is_bounded_and_lossless_for_u128_max() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(u128::MAX, Ok(0));
    let telemetry = sampler.sample_with_reading(Ok(1));
    let json = serde_json::to_value(&telemetry).unwrap();
    assert_eq!(
        json["process_instance"],
        "340282366920938463463374607431768211455"
    );
    assert_eq!(serde_json::from_value::<CpuTelemetry>(json).unwrap(), telemetry);

    for invalid in ["0", "01", "", "1000000000000000000000000000000000000000"] {
        let value = serde_json::json!({
            "metrics_version": 1,
            "process_instance": invalid,
            "source": "child_process_accounting",
            "process_cpu_time": {"state":"unknown","reason":"clock_read_failed","last_known_lower_bound":null}
        });
        let decoded = serde_json::from_value::<CpuTelemetry>(value).unwrap();
        assert_eq!(decoded.validate(), Err(MetricError::InvalidIdentityOrVersion));
    }
}

#[test]
fn missing_process_instance_is_unknown_without_serializing_zero_identity() {
    let mut sampler = CpuAttemptSampler::begin_with_sample(0, Ok(100));
    let telemetry = sampler.sample_with_reading(Ok(120));
    assert_eq!(telemetry.process_instance, None);
    assert_eq!(
        telemetry.process_cpu_time,
        CpuTimeValue::Unknown {
            reason: UnknownReason::MissingProcessInstance,
            last_known_lower_bound: None,
        }
    );
    telemetry.validate().unwrap();
    let value = serde_json::to_value(&telemetry).unwrap();
    assert!(value["process_instance"].is_null());
    assert!(serde_json::from_value::<CpuTelemetry>(value).is_ok());
}

#[test]
fn missing_identity_cannot_claim_a_prior_sample_as_its_lower_bound() {
    let telemetry = CpuTelemetry {
        metrics_version: METRICS_VERSION,
        process_instance: None,
        source: MetricSource::ChildProcessAccounting,
        process_cpu_time: CpuTimeValue::Unknown {
            reason: UnknownReason::MissingProcessInstance,
            last_known_lower_bound: Some(KnownCpuTime {
                start_ns: 10,
                end_ns: 20,
                delta_ns: 10,
            }),
        },
    };
    assert_eq!(
        telemetry.validate(),
        Err(MetricError::InvalidIdentityOrVersion)
    );
}

fn known(telemetry: &CpuTelemetry) -> KnownCpuTime {
    match telemetry.process_cpu_time {
        CpuTimeValue::Known { cumulative } => cumulative,
        _ => panic!("expected known sample"),
    }
}
