#[path = "../src/capacity_read.rs"]
mod capacity_read;

pub use mycity_storage::{Error, Store};
use capacity_read::{
    observe, ReadError, TimelinePresence, BOXED_RECORD_HANDLE_BYTES, COUNTER_SCRATCH_BYTES,
    OBSERVATION_VALUE_BYTES, ORIGINAL_RECORD_BYTES, OBSERVATION_STAMP_BYTES,
    PROJECTION_BYTES, SELECTED_COMPONENT_BYTES,
};
use mycity_core::{
    timeline::Command as TimelineCommand,
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use sha2::{Digest, Sha256};

static NEXT: AtomicU64 = AtomicU64::new(0);
const RULES: &str = "issue-118-capacity-read-v1";
const APPROVED_TEST_BASE: &str = "/Volumes/HP P900/dev/MyCity/work";
const TEST_ROOT_ENV: &str = "MYCITY_CAPACITY_READ_TEST_ROOT";

struct Temp(PathBuf);
impl Temp {
    fn new(test_name: &str) -> Self {
        let approved_base = fs::canonicalize(APPROVED_TEST_BASE).unwrap_or_else(|error| {
            panic!("required existing HP P900 test base is unavailable: {APPROVED_TEST_BASE}: {error}")
        });
        let configured_root = std::env::var_os(TEST_ROOT_ENV)
            .unwrap_or_else(|| panic!("set {TEST_ROOT_ENV} to an existing per-run directory under {APPROVED_TEST_BASE}"));
        let configured_root = PathBuf::from(configured_root);
        assert!(
            configured_root.is_absolute(),
            "{TEST_ROOT_ENV} must be an absolute path"
        );
        let root = fs::canonicalize(&configured_root).unwrap_or_else(|error| {
            panic!("{TEST_ROOT_ENV} must name an existing directory: {}: {error}", configured_root.display())
        });
        assert!(
            root.is_dir(),
            "{TEST_ROOT_ENV} must resolve to an existing directory: {}",
            root.display()
        );
        assert!(
            root != approved_base && root.starts_with(&approved_base),
            "{TEST_ROOT_ENV} must resolve strictly below {} (resolved to {})",
            approved_base.display(),
            root.display()
        );

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after UNIX epoch")
            .as_nanos();
        let child_name = format!(
            "{test_name}-{}-{}-{nonce}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let p = root.join(child_name);
        fs::create_dir(&p).unwrap_or_else(|error| {
            panic!("cannot create unique retained test directory {}: {error}", p.display())
        });
        println!("capacity_read retained test data: {}", p.display());
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}

fn create(path: &Path) -> Store {
    Store::create(path, include_bytes!("../../../fixtures/m0-e0-opening.json"), vec![Resource::new(41, 8).unwrap()], RULES).unwrap()
}
fn env(s: &Store, id: u64, action: Action) -> Envelope {
    Envelope { world: 1, branch: 1, operation_id: id, expected_revision: s.engine().unwrap().world().view().revision, actions: vec![action] }
}
fn empty_transfer(s: &Store) -> Action {
    Action::Transfer {
        from: s.engine().unwrap().world().account_ref(2).unwrap(),
        to: s.engine().unwrap().world().account_ref(3).unwrap(),
        amount: Money::ZERO,
    }
}

fn literal_counter_oracle(resources: &[Resource], leases: &[Option<mycity_core::transaction::Lease>]) -> (usize, bool) {
    let count = leases.iter().flatten().count();
    let mut reserved = vec![0u64; resources.len()];
    let mut occupied = vec![0u64; resources.len()];
    for lease in leases.iter().flatten() {
        let Some(i) = resources.iter().position(|r| r.id == lease.resource && r.id != 0) else { return (count, false); };
        let sums = if lease.occupied { &mut occupied } else { &mut reserved };
        sums[i] += u64::from(lease.units);
    }
    let consistent = resources.iter().enumerate().all(|(i, r)| reserved[i] == u64::from(r.reserved) && occupied[i] == u64::from(r.occupied));
    (count, consistent)
}

#[test]
fn projection_size_is_an_expression_and_original_record_is_separately_sized() {
    assert!(PROJECTION_BYTES <= 16 * 1024, "root must verify expression against target ABI");
    assert!(ORIGINAL_RECORD_BYTES > 0);
    println!(
        "projection_bytes={PROJECTION_BYTES} stamp_bytes={OBSERVATION_STAMP_BYTES} \
         counter_scratch_bytes={COUNTER_SCRATCH_BYTES} \
         selected_component_bytes={SELECTED_COMPONENT_BYTES} \
         observation_value_bytes={OBSERVATION_VALUE_BYTES} \
         box_handle_bytes={BOXED_RECORD_HANDLE_BYTES} original_record_bytes={ORIGINAL_RECORD_BYTES}; \
         selected component sum is not a stack peak or upper bound"
    );
}

#[test]
fn hot_archive_rejected_and_missing_receipts_keep_original_semantics() {
    let t = Temp::new("hot_archive_rejected_and_missing_receipts_keep_original_semantics");
    let mut s = create(&t.world());
    let hot_action = empty_transfer(&s);
    let hot_envelope = env(&s, 1, hot_action);
    assert!(s.execute(Caller::System, hot_envelope.clone()).unwrap().model.is_ok());
    let hot_original = *s.receipt(1).unwrap().as_ref().unwrap();
    let literal_sha: [u8; 32] = Sha256::digest(serde_json::to_vec(&hot_envelope).unwrap()).into();
    assert_eq!(literal_sha, hot_original.payload_sha256);
    assert_eq!(hot_original.actor, 0); // Literal caller authority: System.
    assert_eq!(hot_original.actions[0], Some(hot_envelope.actions[0]));
    assert!(hot_original.actions.iter().skip(1).all(Option::is_none));
    let hot = observe(&mut s, Some(1)).unwrap();
    assert_eq!(hot.projection.timeline_presence, TimelinePresence::Absent);
    assert!(hot.projection.progress.is_none());
    let hot_record = hot.receipt.unwrap();
    assert_eq!(hot_record.operation_id, hot_original.operation_id);
    assert_eq!(hot_record.actor, hot_original.actor);
    assert_eq!(hot_record.expected_revision, hot_original.expected_revision);
    assert_eq!(hot_record.payload_sha256, hot_original.payload_sha256);
    assert_eq!(hot_record.actions, hot_original.actions);
    assert_eq!(hot_record.outcome, hot_original.outcome);
    assert_eq!(hot.units_aliases_windows_rights_future_admission.future_admission, capacity_read::Unknown::Unknown);

    let failed = env(&s, 2, empty_transfer(&s));
    let mut failed = failed;
    failed.expected_revision += 99;
    let failure_result = s.execute(Caller::System, failed).unwrap();
    assert!(failure_result.model.is_err());
    let rejected = observe(&mut s, Some(2)).unwrap().receipt.unwrap();
    assert!(rejected.outcome.is_err());
    assert_eq!(rejected.expected_revision, s.engine().unwrap().journal().last().unwrap().expected_revision);
    assert_eq!(rejected.actor, s.engine().unwrap().journal().last().unwrap().actor);
    assert_eq!(rejected.payload_sha256, s.engine().unwrap().journal().last().unwrap().payload_sha256);
    let missing = observe(&mut s, Some(9999)).unwrap();
    assert!(missing.receipt.is_none());

    let world = t.world();
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&world, RULES).unwrap();
    let archived = observe(&mut s, Some(1)).unwrap().receipt.unwrap();
    assert_eq!(archived.operation_id, hot_original.operation_id);
    assert_eq!(archived.actor, hot_original.actor);
    assert_eq!(archived.payload_sha256, hot_original.payload_sha256);
    assert_eq!(archived.actions, hot_original.actions);
}

#[test]
fn real_resource_lease_rows_match_independent_literal_counter_oracle_after_release() {
    let t = Temp::new("real_resource_lease_rows_match_independent_literal_counter_oracle_after_release");
    let mut s = create(&t.world());
    let reserve = env(&s, 1, Action::ReserveCapacity { resource: 41, units: 3 });
    assert!(s.execute(Caller::System, reserve).unwrap().model.is_ok());
    let token = s.engine().unwrap().journal().last().unwrap().outcome.unwrap().created_tokens[0].unwrap();
    let observed = observe(&mut s, None).unwrap();
    let (count, consistent) = literal_counter_oracle(&observed.projection.resources[..observed.projection.resource_rows], &observed.projection.leases[..observed.projection.lease_slots]);
    assert_eq!(count, observed.projection.lease_counters.checked_lease_count);
    assert!(consistent);
    assert!(observed.projection.lease_counters.consistent());
    assert_eq!(observed.projection.resources.iter().find(|r| r.id == 41).unwrap().reserved, 3);

    let occupy = env(&s, 2, Action::OccupyCapacity { token });
    assert!(s.execute(Caller::System, occupy).unwrap().model.is_ok());
    let released = env(&s, 3, Action::ReleaseCapacity { token });
    assert!(s.execute(Caller::System, released).unwrap().model.is_ok());
    let final_observation = observe(&mut s, Some(3)).unwrap();
    assert_eq!(final_observation.projection.lease_counters.checked_lease_count, 0);
    assert_eq!(final_observation.projection.resources.iter().find(|r| r.id == 41).unwrap().reserved, 0);
    assert_eq!(final_observation.projection.resources.iter().find(|r| r.id == 41).unwrap().occupied, 0);
    assert!(final_observation.receipt.unwrap().outcome.is_ok());
}

#[test]
fn partial_timeline_progress_is_returned_raw_and_checkpointed() {
    let t = Temp::new("partial_timeline_progress_is_returned_raw_and_checkpointed");
    let mut s = create(&t.world());
    for (id, command) in [
        TimelineCommand::Enable,
        TimelineCommand::Claims { first_id: 1, count: 300, payer: 3, payee: 2, amount: Money::new(1).unwrap(), due: 3600 },
        TimelineCommand::Advance { target: 3600, limit: 256 },
    ].into_iter().enumerate() {
        let e = env(&s, id as u64 + 1, Action::Timeline { command });
        s.execute(Caller::System, e).unwrap().model.unwrap();
    }
    let raw = s.engine().unwrap().timeline().unwrap().progress;
    assert!(raw.target.is_some());
    let observed = observe(&mut s, None).unwrap();
    assert_eq!(observed.projection.progress, Some(raw));
    assert_eq!(observed.projection.timeline_presence, TimelinePresence::Partial);
    s.checkpoint().unwrap();
    let after = observe(&mut s, None).unwrap();
    assert_eq!(after.projection.progress, Some(raw));
}

#[test]
fn poisoned_store_read_preserves_recovery_required_error() {
    use mycity_storage::CheckpointPoint;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let t = Temp::new("poisoned_store_read_preserves_recovery_required_error");
    let mut s = create(&t.world());
    let request = env(&s, 1, empty_transfer(&s));
    assert!(s.execute(Caller::System, request).unwrap().model.is_ok());
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = s.checkpoint_with_observer(|p| if p == CheckpointPoint::SnapshotSynced { panic!("interrupt") } else { Ok(()) });
    }));
    assert!(result.is_err());
    assert!(matches!(observe(&mut s, Some(1)), Err(ReadError::Store(mycity_storage::Error::RecoveryRequired))));
}
