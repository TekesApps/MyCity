use mycity_core::{
    transaction::{Action, Caller, Envelope, TxError},
    units::Money,
};
use mycity_storage::{CheckpointPoint, Store};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static ID: AtomicU64 = AtomicU64::new(0);
const RULES: &str = "m0-storage-process-fixture-v1";
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-checkpoint-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn create(t: &Temp) -> Store {
    Store::create(
        &t.world(),
        include_bytes!("../../../fixtures/m0-e0-opening.json"),
        vec![],
        RULES,
    )
    .unwrap()
}
fn env(s: &Store, id: u64) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: id,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![Action::Transfer {
            from: s.engine().unwrap().world().account_ref(2).unwrap(),
            to: s.engine().unwrap().world().account_ref(3).unwrap(),
            amount: Money::new(0).unwrap(),
        }],
    }
}
#[test]
fn more_than_two_hot_windows_exact_success_and_failure_retry_after_restart() {
    let t = Temp::new();
    let mut s = create(&t);
    let success = env(&s, 1);
    let expected = s.execute(Caller::System, success.clone()).unwrap().model;
    let mut failure = env(&s, 2);
    failure.expected_revision = 999;
    let failed = s.execute(Caller::System, failure.clone()).unwrap().model;
    assert_eq!(failed, Err(TxError::StaleRevision));
    for id in 3..=2100 {
        let e = env(&s, id);
        assert!(s.execute(Caller::System, e).unwrap().model.is_ok());
    }
    let hash = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    assert_eq!(seq, 2100);
    assert!(s.engine().unwrap().journal().len() < 1024);
    s.checkpoint().unwrap();
    s.checkpoint().unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(
        s.execute(Caller::System, success.clone()).unwrap().model,
        expected
    );
    assert_eq!(s.execute(Caller::System, failure).unwrap().model, failed);
    let mut changed = success.clone();
    changed.expected_revision = 123;
    assert_eq!(
        s.execute(Caller::System, changed).unwrap().model,
        Err(TxError::OperationConflict)
    );
    assert_eq!(
        s.execute(Caller::Account(2), success).unwrap().model,
        Err(TxError::Unauthorized)
    );
    assert_eq!(s.committed_sequence(), seq);
    assert_eq!(s.state_hash().unwrap(), hash);
    println!(
        "checkpoint_boundary: commits=2100 original_hot_limit=1024 sequence={seq} status={}",
        serde_json::to_string(&s.checkpoint_status().unwrap()).unwrap()
    );
}
#[test]
fn actual_process_exit_at_every_checkpoint_publication_boundary() {
    for point in [
        CheckpointPoint::SnapshotSynced,
        CheckpointPoint::SegmentSynced,
        CheckpointPoint::RootPublished,
        CheckpointPoint::CurrentRenamed,
        CheckpointPoint::CurrentSynced,
        CheckpointPoint::Retired,
    ] {
        let t = Temp::new();
        let mut s = create(&t);
        let e = env(&s, 1);
        let receipt = s.execute(Caller::System, e.clone()).unwrap().model;
        let hash = s.state_hash().unwrap();
        drop(s);
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
            .args([
                "checkpoint",
                t.world().to_str().unwrap(),
                &format!("{point:?}"),
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(97),
            "{point:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut s = Store::open(&t.world(), RULES).unwrap();
        assert_eq!(s.state_hash().unwrap(), hash);
        assert_eq!(s.execute(Caller::System, e).unwrap().model, receipt);
        assert_eq!(s.committed_sequence(), 1);
        s.checkpoint().unwrap();
        drop(s);
        Store::open(&t.world(), RULES).unwrap();
    }
    println!("checkpoint_actual_process_exits=6");
}
#[test]
fn committed_archive_corruption_stops_recovery_and_does_not_fall_back() {
    let t = Temp::new();
    let mut s = create(&t);
    let e = env(&s, 1);
    s.execute(Caller::System, e).unwrap();
    s.checkpoint().unwrap();
    drop(s);
    let p = fs::read_dir(t.world())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("checkpoint-")
        })
        .unwrap();
    fs::write(p, b"{}").unwrap();
    assert!(Store::open(&t.world(), RULES).is_err());
}

#[test]
fn retry_at_exact_rotation_threshold_does_not_become_a_new_operation() {
    let t = Temp::new();
    let mut s = create(&t);
    let first = env(&s, 1);
    let expected = s.execute(Caller::System, first.clone()).unwrap().model;
    let mut failed = env(&s, 2);
    failed.expected_revision = 0;
    let failure = s.execute(Caller::System, failed.clone()).unwrap().model;
    for id in 3..=512 {
        let e = env(&s, id);
        s.execute(Caller::System, e).unwrap();
    }
    let hash = s.state_hash().unwrap();
    assert_eq!(s.engine().unwrap().journal().len(), 512);
    assert_eq!(s.execute(Caller::System, first).unwrap().model, expected);
    assert_eq!(s.execute(Caller::System, failed).unwrap().model, failure);
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), 512);
}
