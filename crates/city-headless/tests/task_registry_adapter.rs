#[path = "../src/task_registry_adapter.rs"]
mod task_registry_adapter;

use mycity_core::{
    air::{Input, State},
    arena::Scope,
    background::{Background, Failure, Job},
    task_registry::{CachedOutput, ClockKind, ScopeIdentity, WorkWindow},
};
use serde_json::json;
use task_registry_adapter::{
    observe_required, result_matches_pending, ActualJobBinding, HardBlockReason, HintState,
    NoHintReason, OwnerView, UnknownReason,
};

const EMPTY_AIR_INPUT: &[u8] = include_bytes!("../../../tests/fixtures/issue-101/air-empty-sources.json");

fn input() -> Input {
    Input::parse(EMPTY_AIR_INPUT).unwrap()
}

/// Construct the actual public Background deserialization shape, then validate
/// it through Background::validate before tests inspect Background::pending().
fn actual_background(attempt: u8, incarnation: u64, running: bool, failure: Option<Failure>) -> Background {
    let input = input();
    let digest = State::new(input.clone()).unwrap().checkpoint_digest().unwrap().1;
    let job = Job {
        id: 1,
        base_revision: 17,
        field_version: 0,
        start: 0,
        end: 60,
        input,
        checkpoint_sha256: digest,
        incarnation,
        generation: u32::from(attempt),
        attempt,
        running,
        failure,
    };
    let background: Background = serde_json::from_value(json!({
        "field": null,
        "pending": job,
        "version": 0,
        "next_job": 2,
        "incarnation": incarnation
    })).unwrap();
    background.validate().unwrap();
    background
}

fn owner(token: Option<u64>, world_time_seconds: u64) -> OwnerView {
    OwnerView {
        scope: Scope::new(7, 3).unwrap(),
        city_id: None,
        world_time_seconds,
        snapshot_token: token,
    }
}

fn scope(world: u64, branch: u64, city_id: Option<u32>) -> ScopeIdentity {
    ScopeIdentity { world_id: world, branch_id: branch, city_id }
}

#[test]
fn actual_pending_job_with_empty_sources_proposes_the_positive_interval() {
    let background = actual_background(0, 0, false, None);
    let job = background.pending().unwrap();
    assert!(job.input.sources.is_empty());
    let first = observe_required(&background, owner(Some(1), 60), &[]);
    assert_eq!(first.state, HintState::Candidate);
    assert_eq!(first.business_key.unwrap().source_id, job.id);
    assert_eq!(first.business_key.unwrap().source_version, 1);
    assert_eq!(first.business_key.unwrap().window, WorkWindow { start: 0, end: 60 });
    assert_eq!(first.attempt.as_ref().unwrap().field_version, 0);
    assert_eq!(first.attempt.as_ref().unwrap().base_revision, job.base_revision);
    assert_eq!(first.attempt.as_ref().unwrap().input_identity, job.input.identity());
    assert_eq!(first.attempt.as_ref().unwrap().checkpoint_sha256, job.checkpoint_sha256);
    assert_eq!(first.candidate.unwrap().clock, ClockKind::World);
    assert_eq!(first.snapshot_token, Some(1));
    assert!(!first.snapshot_token_durable);
    let repeated = observe_required(&background, owner(Some(1), 60), &[]);
    assert_eq!(first, repeated);
}

#[test]
fn missing_owner_token_is_unknown_and_never_creates_durable_identity() {
    let background = actual_background(0, 0, false, None);
    let observed = observe_required(&background, owner(None, 60), &[]);
    assert_eq!(observed.state, HintState::Unknown(UnknownReason::MissingOwnerSnapshotToken));
    assert!(observed.business_key.is_some());
    assert!(!observed.snapshot_token_durable);
}

#[test]
fn transient_and_cancelled_attempts_keep_one_stable_business_key() {
    let first = actual_background(1, 1, false, Some(Failure::Transient));
    let second = actual_background(2, 2, false, Some(Failure::Cancelled));
    let a = observe_required(&first, owner(Some(10), 60), &[]);
    let b = observe_required(&second, owner(Some(11), 60), &[]);
    assert_eq!(a.business_key, b.business_key);
    assert_ne!(a.attempt, b.attempt);
    assert_eq!(a.state, HintState::Candidate);
    assert_eq!(b.state, HintState::Candidate);
    assert!(!result_matches_pending(a.attempt.as_ref().unwrap(), scope(7, 3, None), second.pending()));
    assert!(result_matches_pending(b.attempt.as_ref().unwrap(), scope(7, 3, None), second.pending()));
}

#[test]
fn recovered_same_business_job_has_new_generation_and_rejects_old_late_result() {
    let before_recovery = actual_background(1, 1, true, None);
    let after_recovery = actual_background(2, 2, true, None);
    let old = observe_required(&before_recovery, owner(Some(20), 60), &[]);
    let current = observe_required(&after_recovery, owner(Some(21), 60), &[]);
    assert_eq!(old.business_key, current.business_key);
    assert_ne!(old.attempt, current.attempt);
    assert_eq!(old.state, HintState::NoHint(NoHintReason::AlreadyRunning));
    assert_eq!(current.state, HintState::NoHint(NoHintReason::AlreadyRunning));
    assert!(!result_matches_pending(old.attempt.as_ref().unwrap(), scope(7, 3, None), after_recovery.pending()));
    assert!(result_matches_pending(current.attempt.as_ref().unwrap(), scope(7, 3, None), after_recovery.pending()));
}

#[test]
fn identical_real_job_binding_cannot_cross_world_branch_or_city_scope() {
    let background = actual_background(1, 1, true, None);
    let job = background.pending();
    let observed = observe_required(&background, owner(Some(22), 60), &[]);
    let binding = observed.attempt.as_ref().unwrap();

    assert!(result_matches_pending(binding, scope(7, 3, None), job));
    assert!(!result_matches_pending(binding, scope(8, 3, None), job));
    assert!(!result_matches_pending(binding, scope(7, 4, None), job));
    assert!(!result_matches_pending(binding, scope(7, 3, Some(9)), job));
    assert_eq!(binding.clock, ClockKind::World);
}

#[test]
fn non_covering_cache_cannot_suppress_existing_required_job() {
    let background = actual_background(0, 0, false, None);
    let scope = ScopeIdentity { world_id: 7, branch_id: 3, city_id: None };
    let wrong_range = [CachedOutput {
        scope,
        source_id: 1,
        source_version: 1,
        clock: ClockKind::World,
        observed_at: 60,
        coverage: WorkWindow { start: 60, end: 120 },
        valid: true,
    }];
    let observed = observe_required(&background, owner(Some(30), 60), &wrong_range);
    assert_eq!(observed.state, HintState::Candidate);
    assert_eq!(observed.candidate.unwrap().key.source_id, background.pending().unwrap().id);
}

#[test]
fn attempt_hard_gates_are_reported_without_erasing_pending_obligation() {
    let exhausted = actual_background(3, 3, false, Some(Failure::Transient));
    let invalid = actual_background(1, 1, false, Some(Failure::InvalidInput));
    let exhausted_view = observe_required(&exhausted, owner(Some(40), 60), &[]);
    let invalid_view = observe_required(&invalid, owner(Some(41), 60), &[]);
    assert_eq!(exhausted_view.state, HintState::HardBlocked(HardBlockReason::AttemptLimitReached));
    assert_eq!(invalid_view.state, HintState::HardBlocked(HardBlockReason::InvalidInputFailure));
    assert!(exhausted.pending().is_some());
    assert!(invalid.pending().is_some());
}

#[test]
fn running_job_is_only_an_in_flight_hint_suppression_and_future_due_is_not_dispatched() {
    let running = actual_background(1, 1, true, None);
    let observed_running = observe_required(&running, owner(Some(50), 60), &[]);
    assert_eq!(observed_running.state, HintState::NoHint(NoHintReason::AlreadyRunning));
    assert!(running.pending().is_some());
    let pending = actual_background(0, 0, false, None);
    let before_due = observe_required(&pending, owner(Some(51), 59), &[]);
    assert_eq!(before_due.state, HintState::NoHint(NoHintReason::NotYetDue));
    assert!(pending.pending().is_some());
}
