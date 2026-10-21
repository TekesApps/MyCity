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
fn publication_callback_unwind_requires_recovery_and_preserves_committed_receipt() {
    for point in [WritePoint::CurrentSynced, WritePoint::Applied] {
        let t = Temp::new();
        let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
        let cmd = command(&s);
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = s.execute_with_observer(Caller::Account(2), cmd.clone(), |at| {
                if at == point {
                    panic!("durable callback unwind");
                }
                Ok(())
            });
        }));
        assert!(failed.is_err());
        assert!(matches!(s.engine(), Err(Error::RecoveryRequired)));
        assert!(matches!(
            s.execute(Caller::Account(2), cmd.clone()),
            Err(Error::RecoveryRequired)
        ));
        drop(s);
        let mut reopened = Store::open(&t.world(), "rules1").unwrap();
        assert_eq!(reopened.committed_sequence(), 1);
        let before = reopened.engine().unwrap().state_hash();
        let retry = reopened.execute(Caller::Account(2), cmd).unwrap();
        assert!(retry.has_persistent_receipt);
        assert!(retry.model.is_ok());
        assert_eq!(reopened.engine().unwrap().state_hash(), before);
        assert_eq!(reopened.committed_sequence(), 1);
    }
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

#[test]
fn regional_clocks_migrants_debt_and_snapshot_watermarks_recover_atomically_once() {
    let t = Temp::new();
    let input = include_bytes!("../../../fixtures/m0-region-opening.json");
    let mut s = Store::create(&t.world(), input, vec![], "regional-rules1").unwrap();
    let cmd = Envelope {
        world: 1,
        branch: 1,
        operation_id: 1,
        expected_revision: 0,
        actions: vec![
            Action::RegionMigrate {
                from: 1,
                to: 2,
                requested: 2000,
                tolerance: 2,
            },
            Action::RegionAdvance { seconds: 30 },
            Action::RegionActivate { city: 1 },
        ],
    };
    let receipt = s.execute(Caller::System, cmd.clone()).unwrap();
    assert!(receipt.has_persistent_receipt);
    assert_eq!(receipt.model.unwrap().migrated[0], Some(1998));
    let before = s.engine().unwrap().state_hash();
    let r = s.engine().unwrap().world().view().region.unwrap();
    assert_eq!(r.city(2).unwrap().population, 2098);
    assert_eq!(r.obligations[0].unwrap().arrears.value(), 600000);
    drop(s);
    let mut s = Store::open(&t.world(), "regional-rules1").unwrap();
    assert_eq!(s.engine().unwrap().state_hash(), before);
    assert!(
        s.execute(Caller::System, cmd)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(s.committed_sequence(), 1);
    assert_eq!(s.engine().unwrap().state_hash(), before);
}

#[test]
fn regional_process_exit_never_publishes_half_population_half_clock_or_half_payment() {
    for (point, committed) in [
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
    ] {
        let t = Temp::new();
        let path = t.world();
        let p = path.to_str().unwrap();
        assert!(worker(&["init-regional", p]).status.success());
        let out = worker(&["commit", p, point]);
        assert_eq!(
            out.status.code(),
            Some(97),
            "{point}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let out = worker(&["inspect", p]);
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let r = &v["world"]["region"];
        assert_eq!(v["sequence"], u64::from(committed), "{point}");
        assert_eq!(r["interaction_time"], if committed { 30 } else { 0 });
        assert_eq!(r["active_city"], if committed { 1 } else { 2 });
        assert_eq!(
            r["cities"][0]["population"],
            if committed { 2002 } else { 4000 }
        );
        assert_eq!(
            r["cities"][1]["population"],
            if committed { 2098 } else { 100 }
        );
        assert_eq!(
            r["obligations"][0]["arrears"],
            if committed { 600000 } else { 0 }
        );
        assert_eq!(
            v["world"]["accounts"][2]["cash"],
            if committed { 0 } else { 600000 }
        );
        if committed {
            assert!(worker(&["commit", p, "none"]).status.success());
            let out = worker(&["inspect", p]);
            let after: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(after, v, "{point} retry");
        }
    }
}

fn rewrite_committed_frame(world: &std::path::Path, payload: &[u8]) -> usize {
    use sha2::{Digest, Sha256};
    let length = (payload.len() as u32).to_le_bytes();
    let mut hash = Sha256::new();
    hash.update(length);
    hash.update(payload);
    let checksum = hash.finalize();
    let mut wal = length.to_vec();
    wal.extend_from_slice(payload);
    wal.extend_from_slice(&checksum);
    let committed = wal.len();
    wal.extend_from_slice(b"uncommitted fault evidence");
    fs::write(world.join("wal.bin"), &wal).unwrap();
    let mut current: serde_json::Value =
        serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
    let slot = current["slot"].as_u64().unwrap();
    let root_path = world.join(format!("root-{slot}.json"));
    let mut root: serde_json::Value =
        serde_json::from_slice(&fs::read(&root_path).unwrap()).unwrap();
    root["wal_end"] = serde_json::json!(committed);
    root["wal_chain"] = serde_json::json!(format!("{checksum:x}"));
    let bytes = serde_json::to_vec(&root).unwrap();
    current["root_sha256"] = serde_json::json!(mycity_core::sha256(&bytes));
    fs::write(root_path, bytes).unwrap();
    fs::write(
        world.join("CURRENT.json"),
        serde_json::to_vec(&current).unwrap(),
    )
    .unwrap();
    wal.len()
}
#[test]
fn replay_preserves_old_value_last_key_and_rejects_ignored_unknown_fields_before_tail_gc() {
    for mode in 0..5 {
        let t = Temp::new();
        let mut s = Store::create(&t.world(), INPUT, vec![], "rules1").unwrap();
        let request = command(&s);
        s.execute(Caller::Account(2), request.clone()).unwrap();
        let expected = s.engine().unwrap().export_checkpoint().unwrap();
        drop(s);
        let wal = fs::read(t.world().join("wal.bin")).unwrap();
        let n = u32::from_le_bytes(wal[..4].try_into().unwrap()) as usize;
        let value: serde_json::Value = serde_json::from_slice(&wal[4..4 + n]).unwrap();
        let original = serde_json::to_string(&value["record"]).unwrap();
        let raw = match mode {
            0 => format!("{{{}{}", "\"\\u0061ctor\":999,".repeat(70), &original[1..]),
            1 => {
                let mut r = value["record"].clone();
                r["unknown"] = serde_json::json!(0);
                serde_json::to_string(&r).unwrap()
            }
            2 => {
                let mut r = value["record"].clone();
                r["outcome"]["Ok"]["unknown"] = serde_json::json!(0);
                serde_json::to_string(&r).unwrap()
            }
            3 => format!("{{\"actor\":1e400,{}", &original[1..]),
            _ => format!("{{\"actor\":{{\"nested\":1e400}},{}", &original[1..]),
        };
        if mode >= 3 {
            assert!(serde_json::from_str::<serde_json::Value>(&raw).is_err());
        }
        let previous = value["previous"].as_str().unwrap();
        let escaped = previous
            .bytes()
            .map(|b| format!("\\u{b:04x}"))
            .collect::<Vec<_>>()
            .join("");
        let payload = format!(
            "{{\"record\":{raw},\"envelope\":{},\"previous\":\"{escaped}\",\"sequence\":1}}",
            value["envelope"]
        );
        let untrimmed = rewrite_committed_frame(&t.world(), payload.as_bytes());
        if mode == 0 {
            let mut s = Store::open(&t.world(), "rules1").unwrap();
            assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), expected);
            assert_eq!(
                s.execute(Caller::Account(2), request)
                    .unwrap()
                    .committed_sequence,
                1
            );
        } else {
            assert!(Store::open(&t.world(), "rules1").is_err());
            assert_eq!(
                fs::metadata(t.world().join("wal.bin")).unwrap().len() as usize,
                untrimmed
            );
        }
    }
}
