use mycity_core::{
    economy::{Command as Economic, DAY, HOUR},
    transaction::{Action, Caller, Envelope},
};
use mycity_storage::{Store, WritePoint};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-economy-opening.json");
const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-econ-store-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn envelope(s: &Store, command: Economic) -> Envelope {
    let e = s.engine().unwrap();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: e.world().view().revision,
        actions: vec![Action::Economy { command }],
    }
}
fn execute(s: &mut Store, c: Economic) {
    let e = envelope(s, c);
    assert!(s.execute(Caller::System, e).unwrap().model.is_ok());
}
fn advance(s: &mut Store, time: u64) {
    loop {
        let t = s.engine().unwrap().world().view().economy.unwrap().time;
        if t == time {
            break;
        }
        execute(
            s,
            Economic::Advance {
                seconds: (time - t).min(HOUR) as u32,
            },
        );
    }
}
fn clone_dir(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for item in fs::read_dir(from).unwrap() {
        let p = item.unwrap().path();
        if p.file_name().unwrap() == "LOCK" {
            continue;
        }
        fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
    }
}
#[test]
fn economic_transactions_survive_process_exit_at_every_wal_and_response_boundary() {
    let t = Temp::new();
    let points = [
        "Prepared",
        "WalHeaderHalf",
        "WalPayloadHalf",
        "WalChecksumHalf",
        "WalWritten",
        "WalSynced",
        "RootPublished",
        "CurrentRenamed",
        "CurrentSynced",
        "Applied",
        "Responded",
    ];
    let mut exits = 0;
    for stage in ["raw_dispatch", "goods_delivery", "refund", "day30_tax"] {
        let base = t.0.join(format!("base-{stage}"));
        let mut s = Store::create(&base, INPUT, vec![], RULES).unwrap();
        let c = match stage {
            "raw_dispatch" => Economic::Advance { seconds: 1800 },
            "goods_delivery" => {
                advance(&mut s, 9 * HOUR);
                Economic::Advance { seconds: 3600 }
            }
            "refund" => {
                execute(
                    &mut s,
                    Economic::Prepay {
                        quantity: mycity_core::units::Quantity::new(20000).unwrap(),
                    },
                );
                advance(&mut s, DAY);
                Economic::CancelFuture
            }
            "day30_tax" => {
                advance(&mut s, 30 * DAY - HOUR);
                Economic::Advance { seconds: 3600 }
            }
            _ => unreachable!(),
        };
        let cmd = envelope(&s, c);
        let old = s.state_hash().unwrap();
        let oldseq = s.committed_sequence();
        drop(s);
        let command_path = t.0.join(format!("{stage}.json"));
        fs::write(&command_path, serde_json::to_vec(&cmd).unwrap()).unwrap();
        let reference = t.0.join(format!("reference-{stage}"));
        clone_dir(&base, &reference);
        let mut expected = Store::open(&reference, RULES).unwrap();
        assert!(expected
            .execute(Caller::System, cmd.clone())
            .unwrap()
            .model
            .is_ok());
        let new = expected.state_hash().unwrap();
        drop(expected);
        assert_ne!(old, new);
        for point in points {
            let dir = t.0.join(format!("{stage}-{point}"));
            clone_dir(&base, &dir);
            let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
                .arg("economy-commit")
                .arg(&dir)
                .arg(&command_path)
                .arg(point)
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(97),
                "{stage}/{point}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            exits += 1;
            let mut recovered = Store::open(&dir, RULES).unwrap();
            let committed = matches!(
                point,
                "CurrentRenamed" | "CurrentSynced" | "Applied" | "Responded"
            );
            assert_eq!(
                recovered.state_hash().unwrap(),
                if committed { &new } else { &old }.to_string(),
                "{stage}/{point}"
            );
            assert_eq!(
                recovered.committed_sequence(),
                oldseq + u64::from(committed)
            );
            assert!(recovered
                .execute(Caller::System, cmd.clone())
                .unwrap()
                .model
                .is_ok());
            assert_eq!(
                recovered.state_hash().unwrap(),
                new,
                "retry {stage}/{point}"
            );
            assert_eq!(recovered.committed_sequence(), oldseq + 1);
        }
    }
    assert_eq!(exits, 44);
    println!("actual_process_exits=44 stages=raw_dispatch,goods_delivery,refund,day30_tax; old_or_new_root_and_exactly_once_retry_verified");
}
#[test]
fn economic_io_uncertainty_hides_state_until_reopen_then_retry_is_exactly_once() {
    let t = Temp::new();
    let dir = t.0.join("world");
    let mut s = Store::create(&dir, INPUT, vec![], RULES).unwrap();
    let cmd = envelope(&s, Economic::Advance { seconds: 1800 });
    assert!(s
        .execute_with_observer(Caller::System, cmd.clone(), |p| {
            if p == WritePoint::CurrentRenamed {
                Err(std::io::Error::other("injected uncertain IO"))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(s.engine().is_err());
    drop(s);
    let mut s = Store::open(&dir, RULES).unwrap();
    let hash = s.state_hash().unwrap();
    let e = s.engine().unwrap().world().view().economy.unwrap();
    assert_eq!(e.time, 1800);
    assert!(e.raw_transit.is_some());
    s.execute(Caller::System, cmd).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), 1);
}
