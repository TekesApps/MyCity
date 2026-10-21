use mycity_core::{
    transaction::{Action, Caller, Envelope, Resource, TxError},
    units::Money,
};
use mycity_storage::{CheckpointPoint, Query, Store};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-t13a-full-opening-candidate.json");
const RULES: &str = "checkpoint-v2-regression";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-cpv2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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
fn create(p: &Path) -> Store {
    let r = [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
        (203, 1000),
        (204, 1000),
    ]
    .into_iter()
    .map(|(i, c)| Resource::new(i, c).unwrap())
    .collect();
    Store::create_spatial_v2(
        p,
        INPUT,
        r,
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}
fn movement(s: &Store, id: u64, reverse: bool) -> Envelope {
    let w = s.engine().unwrap().world();
    let (a, b) = if reverse { (3, 2) } else { (2, 3) };
    Envelope {
        world: 1,
        branch: 1,
        operation_id: id,
        expected_revision: w.view().revision,
        actions: vec![Action::Transfer {
            from: w.account_ref(a).unwrap(),
            to: w.account_ref(b).unwrap(),
            amount: Money::new(1).unwrap(),
        }],
    }
}
fn start(s: &mut Store) -> (Envelope, Envelope) {
    let mut failed = movement(s, 1, false);
    failed.expected_revision = 999;
    assert_eq!(
        s.execute(Caller::System, failed.clone()).unwrap().model,
        Err(TxError::StaleRevision)
    );
    let good = movement(s, 2, false);
    assert!(s
        .execute(Caller::System, good.clone())
        .unwrap()
        .model
        .is_ok());
    (failed, good)
}
#[test]
fn borrowed_checkpoint_has_all_fields_and_excludes_only_hot_journal() {
    let t = Temp::new();
    let mut s = create(&t.world());
    start(&mut s);
    let mut all = serde_json::to_value(s.engine().unwrap()).unwrap();
    all["journal"] = serde_json::json!([]);
    let bytes = s.engine().unwrap().export_checkpoint().unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        all
    );
    let restored = mycity_core::transaction::Engine::restore_checkpoint(&bytes).unwrap();
    assert_eq!(restored.export_checkpoint().unwrap(), bytes);
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(s.engine().unwrap().write_checkpoint(Broken).is_err());
}
#[test]
fn multiple_hot_windows_failed_receipts_queries_and_empty_saves_keep_originals() {
    let t = Temp::new();
    let mut s = create(&t.world());
    let (failed, good) = start(&mut s);
    s.checkpoint().unwrap();
    for id in 3..=1042 {
        let e = movement(&s, id, id % 2 == 1);
        assert!(s.execute(Caller::System, e).unwrap().model.is_ok());
        if id % 257 == 0 {
            s.checkpoint().unwrap();
        }
    }
    let bytes = s.engine().unwrap().export_checkpoint().unwrap();
    let seq = s.committed_sequence();
    for _ in 0..3 {
        s.checkpoint().unwrap();
    }
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), bytes);
    assert_eq!(
        s.execute(Caller::System, failed).unwrap().model,
        Err(TxError::StaleRevision)
    );
    assert!(s
        .execute(Caller::System, good.clone())
        .unwrap()
        .model
        .is_ok());
    let mut conflict = good;
    conflict.expected_revision = 77;
    assert_eq!(
        s.execute(Caller::System, conflict).unwrap().model,
        Err(TxError::OperationConflict)
    );
    assert_eq!(s.committed_sequence(), seq);
    assert!(s.receipt(999999).unwrap().is_none());
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let p = s.query(Query::Receipts, cursor.as_deref(), 64).unwrap();
        for r in p.rows {
            seen.push((
                r["committed_sequence"].as_u64().unwrap(),
                r["record"]["operation_id"].as_u64().unwrap(),
            ));
        }
        cursor = p.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 1042);
    assert_eq!(seen, (1..=1042).rev().map(|n| (n, n)).collect::<Vec<_>>());
    let files: Vec<_> = fs::read_dir(t.world())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        files
            .iter()
            .filter(|n| n.starts_with("world-") && !n.starts_with("world-index-"))
            .count(),
        2
    );
    assert_eq!(
        files
            .iter()
            .filter(|n| n.starts_with("world-index-"))
            .count(),
        1
    );
    assert!(!files.iter().any(|n| n == "wal.bin"));
}
#[test]
fn real_process_exit_at_each_v2_publication_and_gc_boundary_keeps_full_state() {
    if let Ok(path) = std::env::var("MYCITY_CPV2_WORLD") {
        let target = std::env::var("MYCITY_CPV2_POINT").unwrap();
        let mut s = Store::open(Path::new(&path), RULES).unwrap();
        s.checkpoint_with_observer(|p| {
            if format!("{p:?}") == target {
                std::process::exit(73)
            }
            Ok(())
        })
        .unwrap();
        panic!("missing crash point");
    }
    let points = [
        CheckpointPoint::WorldSynced,
        CheckpointPoint::ReceiptsSynced,
        CheckpointPoint::ReceiptIndexSynced,
        CheckpointPoint::SnapshotSynced,
        CheckpointPoint::SegmentSynced,
        CheckpointPoint::RootPublished,
        CheckpointPoint::CurrentRenamed,
        CheckpointPoint::CurrentSynced,
        CheckpointPoint::GcFileRemoved,
        CheckpointPoint::Retired,
    ];
    for point in points {
        let t = Temp::new();
        let mut s = create(&t.world());
        let (failed, good) = start(&mut s);
        s.checkpoint().unwrap();
        let e = movement(&s, 3, true);
        s.execute(Caller::System, e).unwrap().model.unwrap();
        let bytes = s.engine().unwrap().export_checkpoint().unwrap();
        let seq = s.committed_sequence();
        drop(s);
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "real_process_exit_at_each_v2_publication_and_gc_boundary_keeps_full_state",
                "--nocapture",
            ])
            .env("MYCITY_CPV2_WORLD", t.world())
            .env("MYCITY_CPV2_POINT", format!("{point:?}"))
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73), "{point:?}");
        let mut s = Store::open(&t.world(), RULES).unwrap();
        assert_eq!(
            s.engine().unwrap().export_checkpoint().unwrap(),
            bytes,
            "{point:?}"
        );
        assert_eq!(s.committed_sequence(), seq);
        assert_eq!(
            s.execute(Caller::System, failed).unwrap().model,
            Err(TxError::StaleRevision)
        );
        assert!(s.execute(Caller::System, good).unwrap().model.is_ok());
        assert_eq!(s.committed_sequence(), seq);
        s.checkpoint().unwrap();
    }
}
#[test]
fn current_world_history_and_receipt_corruption_are_rejected_without_rollback() {
    for (prefix, history) in [
        ("world-", false),
        ("world-", true),
        ("receipts-", false),
        ("receipt-index-", false),
    ] {
        let t = Temp::new();
        let mut s = create(&t.world());
        start(&mut s);
        s.checkpoint().unwrap();
        s.checkpoint().unwrap();
        drop(s);
        let mut paths: Vec<_> = fs::read_dir(t.world())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name().unwrap().to_str().unwrap().starts_with(prefix)
                    && !p
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("world-index-")
            })
            .collect();
        paths.sort();
        let p = if history {
            paths.first().unwrap()
        } else {
            paths.last().unwrap()
        };
        let mut bytes = fs::read(p).unwrap();
        bytes[0] ^= 1;
        fs::write(p, bytes).unwrap();
        assert!(Store::open(&t.world(), RULES).is_err(), "{prefix}");
    }
}

#[test]
fn streamed_checkpoint_unwind_never_leaves_a_usable_stale_store() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for point in [
        CheckpointPoint::WorldSynced,
        CheckpointPoint::ReceiptsSynced,
        CheckpointPoint::RootPublished,
        CheckpointPoint::CurrentSynced,
        CheckpointPoint::Retired,
    ] {
        let t = Temp::new();
        let mut s = create(&t.world());
        let (failed, good) = start(&mut s);
        let before = s.engine().unwrap().export_checkpoint().unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = s.checkpoint_with_observer(|p| {
                if p == point {
                    panic!("checkpoint unwind {point:?}");
                }
                Ok(())
            });
        }))
        .is_err());
        assert!(matches!(
            s.engine(),
            Err(mycity_storage::Error::RecoveryRequired)
        ));
        assert!(matches!(
            s.checkpoint(),
            Err(mycity_storage::Error::RecoveryRequired)
        ));
        drop(s);
        let mut s = Store::open(&t.world(), RULES).unwrap();
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), before);
        assert_eq!(
            s.execute(Caller::System, failed).unwrap().model,
            Err(TxError::StaleRevision)
        );
        assert!(s.execute(Caller::System, good).unwrap().model.is_ok());
        assert_eq!(s.committed_sequence(), 2);
    }
}
#[test]
fn streamed_v2_capacity_rejection_before_any_output_remains_usable() {
    let t = Temp::new();
    let mut s = create(&t.world());
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(mycity_storage::CapacityPolicy {
        total_bytes: used + 32768,
        emergency_bytes: 32768,
    })
    .unwrap();
    assert!(matches!(
        s.checkpoint(),
        Err(mycity_storage::Error::Capacity { .. })
    ));
    assert!(s.engine().is_ok());
}
