use mycity_core::{
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::{Error, Store, WritePoint};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static ID: AtomicU64 = AtomicU64::new(1);
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mycity-storage-test-{}-{}-{n}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
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
fn command(s: &Store) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: 1,
        expected_revision: 0,
        actions: vec![Action::Transfer {
            from: s.engine().unwrap().world().account_ref(2).unwrap(),
            to: s.engine().unwrap().world().account_ref(3).unwrap(),
            amount: Money::new(100).unwrap(),
        }],
    }
}
fn worker(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn acknowledged_success_and_operation_coverage_survive_reopen_without_double_payment() {
    let t = Temp::new();
    let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
    let cmd = command(&s);
    let outcome = s.execute(Caller::Account(2), cmd.clone()).unwrap();
    assert!(outcome.has_persistent_receipt);
    assert!(outcome.model.is_ok());
    let hash = s.engine().unwrap().state_hash();
    let journal = serde_json::to_value(s.engine().unwrap().journal()).unwrap();
    drop(s);
    let mut s = Store::open(&t.world(), "rules1").unwrap();
    assert_eq!(s.engine().unwrap().state_hash(), hash);
    assert_eq!(
        serde_json::to_value(s.engine().unwrap().journal()).unwrap(),
        journal
    );
    assert!(
        s.execute(Caller::Account(2), cmd)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(s.committed_sequence(), 1);
    assert_eq!(s.engine().unwrap().state_hash(), hash);
}

#[test]
fn only_tail_outside_the_validated_committed_boundary_is_truncated() {
    let t = Temp::new();
    let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
    let cmd = command(&s);
    s.execute(Caller::Account(2), cmd).unwrap();
    let hash = s.engine().unwrap().state_hash();
    drop(s);
    let p = t.world().join("wal.bin");
    let end = fs::metadata(&p).unwrap().len();
    OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(b"not-committed-partial-frame")
        .unwrap();
    let s = Store::open(&t.world(), "rules1").unwrap();
    assert_eq!(s.engine().unwrap().state_hash(), hash);
    assert_eq!(fs::metadata(p).unwrap().len(), end);
}

#[test]
fn committed_corruption_or_missing_dependency_never_falls_back_to_old_root() {
    for defect in ["wal", "genesis", "current", "latest_root"] {
        let t = Temp::new();
        let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
        let cmd = command(&s);
        s.execute(Caller::Account(2), cmd).unwrap();
        drop(s);
        assert!(t.world().join("root-0.json").exists());
        match defect {
            "wal" => {
                let mut f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(t.world().join("wal.bin"))
                    .unwrap();
                f.seek(SeekFrom::Start(20)).unwrap();
                let mut b = [0];
                f.read_exact(&mut b).unwrap();
                f.seek(SeekFrom::Start(20)).unwrap();
                f.write_all(&[b[0] ^ 1]).unwrap();
                f.sync_all().unwrap();
            }
            "genesis" => fs::remove_file(t.world().join("genesis.json")).unwrap(),
            "current" => fs::write(t.world().join("CURRENT.json"), b"{}").unwrap(),
            _ => fs::remove_file(t.world().join("root-1.json")).unwrap(),
        }
        let wal_len = fs::metadata(t.world().join("wal.bin")).unwrap().len();
        assert!(Store::open(&t.world(), "rules1").is_err(), "{defect}");
        assert_eq!(
            fs::metadata(t.world().join("wal.bin")).unwrap().len(),
            wal_len
        );
    }
}

#[test]
fn a_second_process_cannot_open_a_world_while_writer_lock_is_held() {
    let t = Temp::new();
    let path = t.world();
    let p = path.to_str().unwrap();
    assert!(worker(&["init", p]).status.success());
    let _held = Store::open(&path, "m0-storage-process-fixture-v1").unwrap();
    let out = worker(&["inspect", p]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Locked"));
}

#[test]
fn injected_io_uncertainty_poisoning_prevents_further_writes_until_recovery() {
    for point in [
        WritePoint::WalHeaderHalf,
        WritePoint::WalSynced,
        WritePoint::CurrentRenamed,
    ] {
        let t = Temp::new();
        let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
        let cmd = command(&s);
        assert!(s
            .execute_with_observer(Caller::Account(2), cmd.clone(), |at| if at == point {
                Err(std::io::Error::from_raw_os_error(28))
            } else {
                Ok(())
            })
            .is_err());
        assert!(matches!(s.engine(), Err(Error::RecoveryRequired)));
        assert!(matches!(
            s.execute(Caller::Account(2), cmd),
            Err(Error::RecoveryRequired)
        ));
        drop(s);
        let recovered = Store::open(&t.world(), "rules1").unwrap();
        assert_eq!(
            recovered.committed_sequence(),
            if point == WritePoint::CurrentRenamed {
                1
            } else {
                0
            }
        );
    }
}

#[test]
fn incompatible_rules_reject_instead_of_recalculating_history() {
    let t = Temp::new();
    drop(Store::create(&t.world(), INPUT, vec![], "rules1").unwrap());
    assert!(Store::open(&t.world(), "rules2").is_err());
}

#[test]
fn oversized_command_is_rejected_before_serializing_and_never_changes_storage() {
    let t = Temp::new();
    let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
    let mut cmd = command(&s);
    cmd.actions = vec![cmd.actions[0]; mycity_core::transaction::MAX_ACTIONS + 1];
    let hash = s.engine().unwrap().state_hash();
    let wal_len = fs::metadata(t.world().join("wal.bin")).unwrap().len();
    let outcome = s
        .execute_with_observer(Caller::System, cmd, |_| {
            panic!("oversized command reached persistence")
        })
        .unwrap();
    assert_eq!(
        outcome.model,
        Err(mycity_core::transaction::TxError::TooManyActions)
    );
    assert!(!outcome.has_persistent_receipt);
    assert_eq!(s.committed_sequence(), 0);
    assert_eq!(s.engine().unwrap().state_hash(), hash);
    assert_eq!(
        fs::metadata(t.world().join("wal.bin")).unwrap().len(),
        wal_len
    );
}

#[test]
fn recovery_reclaims_only_known_unreferenced_atomic_temp_files() {
    let t = Temp::new();
    drop(Store::create(&t.world(), INPUT, vec![], "rules1").unwrap());
    let orphan = t.world().join(".CURRENT.json-123-4.tmp");
    fs::write(&orphan, b"partial").unwrap();
    let user = t.world().join("notes.tmp");
    fs::write(&user, b"preserve").unwrap();
    let _ = Store::open(&t.world(), "rules1").unwrap();
    assert!(!orphan.exists());
    assert_eq!(fs::read(user).unwrap(), b"preserve");
}

#[test]
fn store_ownership_handoff_reopens_immediately_during_parallel_process_spawn() {
    let t = Temp::new();
    drop(Store::create(&t.world(), INPUT, vec![], "rules1").unwrap());
    let child = std::thread::spawn(|| {
        for _ in 0..100 {
            assert!(Command::new("/usr/bin/true").status().unwrap().success());
        }
    });
    for _ in 0..100 {
        drop(Store::open(&t.world(), "rules1").unwrap());
    }
    child.join().unwrap();
}

#[test]
fn abrupt_process_exit_at_each_boundary_retains_exactly_the_published_operation() {
    let cases = [
        ("Prepared", false),
        ("WalHeaderHalf", false),
        ("WalPayloadHalf", false),
        ("WalChecksumHalf", false),
        ("WalWritten", false),
        ("WalSynced", false),
        ("RootPublished", false),
        ("CurrentRenamed", true),
        ("CurrentSynced", true),
        ("Applied", true),
        ("Responded", true),
    ];
    for (point, committed) in cases {
        let t = Temp::new();
        let path = t.world();
        let p = path.to_str().unwrap();
        let init = worker(&["init", p]);
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );
        let killed = worker(&["commit", p, point]);
        assert_eq!(
            killed.status.code(),
            Some(97),
            "{point}: {}",
            String::from_utf8_lossy(&killed.stderr)
        );
        let out = worker(&["inspect", p]);
        assert!(
            out.status.success(),
            "{point}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["sequence"], u64::from(committed), "{point}");
        assert_eq!(v["world"]["revision"], u64::from(committed));
        assert_eq!(
            v["world"]["accounts"][1]["protected"],
            if committed { 10000 } else { 0 }
        );
        assert_eq!(v["resources"][0]["reserved"], if committed { 8 } else { 0 });
        assert_eq!(
            v["journal"].as_array().unwrap().len(),
            usize::from(committed)
        );
        if committed {
            let retry = worker(&["commit", p, "none"]);
            assert!(retry.status.success());
            let out = worker(&["inspect", p]);
            let after: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(after, v, "{point} retry");
        }
    }
}
