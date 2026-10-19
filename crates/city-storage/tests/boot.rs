use mycity_core::{
    boot::{Command as BootCommand, Facility, Phase},
    transaction::{Action, Caller, Envelope},
};
use mycity_storage::{Store, WritePoint};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-boot-opening.json");
const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-boot-store-{}-{}-{}",
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
fn resources() -> Vec<mycity_core::transaction::Resource> {
    [101, 102]
        .into_iter()
        .map(|id| mycity_core::transaction::Resource::new(id, 2).unwrap())
        .collect()
}
fn envelope(s: &Store, command: BootCommand) -> Envelope {
    let e = s.engine().unwrap();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: e.world().view().revision,
        actions: vec![Action::Boot { command }],
    }
}
fn execute(s: &mut Store, command: BootCommand) {
    let e = envelope(s, command);
    assert!(s.execute(Caller::System, e).unwrap().model.is_ok());
}
fn start(facility: Facility) -> BootCommand {
    BootCommand::Start {
        facility,
        reverse_request: true,
    }
}
fn clone_dir(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let p = entry.unwrap().path();
        if p.file_name().unwrap() == "LOCK" {
            continue;
        }
        fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
    }
}
#[test]
fn pump_and_generator_start_complete_fault_and_cleanup_survive_88_real_process_exits() {
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
    for facility in [Facility::Pump, Facility::Generator] {
        for stage in ["start", "complete", "fault", "cleanup"] {
            let label = format!("{facility:?}-{stage}");
            let base = t.0.join(&label);
            let mut s = Store::create(&base, INPUT, resources(), RULES).unwrap();
            if facility == Facility::Generator {
                execute(&mut s, start(Facility::Pump));
                execute(&mut s, BootCommand::Advance { seconds: 60 });
            }
            if stage != "start" {
                execute(&mut s, start(facility));
                execute(&mut s, BootCommand::Advance { seconds: 30 });
            }
            if stage == "cleanup" {
                execute(&mut s, BootCommand::Fault { facility });
            }
            let command = match stage {
                "start" => start(facility),
                "complete" => BootCommand::Advance { seconds: 30 },
                "fault" => BootCommand::Fault { facility },
                "cleanup" => BootCommand::ClearFailure { facility },
                _ => unreachable!(),
            };
            let cmd = envelope(&s, command);
            let old = s.state_hash().unwrap();
            let sequence = s.committed_sequence();
            drop(s);
            let file = t.0.join(format!("{label}.json"));
            fs::write(&file, serde_json::to_vec(&cmd).unwrap()).unwrap();
            let reference = t.0.join(format!("ref-{label}"));
            clone_dir(&base, &reference);
            let mut expected = Store::open(&reference, RULES).unwrap();
            assert!(expected
                .execute(Caller::System, cmd.clone())
                .unwrap()
                .model
                .is_ok());
            let new = expected.state_hash().unwrap();
            assert_ne!(old, new);
            drop(expected);
            for point in points {
                let dir = t.0.join(format!("{label}-{point}"));
                clone_dir(&base, &dir);
                let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
                    .arg("activity-commit")
                    .arg(&dir)
                    .arg(&file)
                    .arg(point)
                    .output()
                    .unwrap();
                assert_eq!(
                    out.status.code(),
                    Some(97),
                    "{label}/{point}:{}",
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
                    if committed { new.clone() } else { old.clone() },
                    "{label}/{point}"
                );
                assert_eq!(
                    recovered.committed_sequence(),
                    sequence + u64::from(committed)
                );
                assert!(recovered
                    .execute(Caller::System, cmd.clone())
                    .unwrap()
                    .model
                    .is_ok());
                assert_eq!(recovered.state_hash().unwrap(), new);
                assert_eq!(recovered.committed_sequence(), sequence + 1);
                let b = recovered.engine().unwrap().world().view().boot.unwrap();
                let j = b.jobs[if facility == Facility::Pump { 0 } else { 1 }];
                assert_eq!(
                    j.phase,
                    match stage {
                        "start" => Phase::Running,
                        "fault" => Phase::Failed,
                        _ => Phase::Idle,
                    }
                );
                if stage == "fault" || stage == "cleanup" {
                    assert!(b.failed_expense.value() > 0);
                    assert_eq!(j.failed, 1);
                }
            }
        }
    }
    assert_eq!(exits, 88);
    println!("actual_boot_process_exits=88; pump_and_generator_start_complete_fault_cleanup; full_old_or_new_root_and_exactly_once_retry_verified");
}
#[test]
fn startup_wip_and_leases_restore_with_immutable_spatial_assets_and_uncertain_io() {
    let t = Temp::new();
    let dir = t.0.join("joint");
    let mut s = Store::create_spatial(
        &dir,
        INPUT,
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap();
    let space = serde_json::to_value(s.spatial_reference().unwrap()).unwrap();
    let cmd = envelope(&s, start(Facility::Pump));
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
    assert_eq!(
        serde_json::to_value(s.spatial_reference().unwrap()).unwrap(),
        space
    );
    assert!(s.execute(Caller::System, cmd).unwrap().model.is_ok());
    assert_eq!(s.state_hash().unwrap(), hash);
    execute(&mut s, BootCommand::Advance { seconds: 30 });
    let hash = s.state_hash().unwrap();
    drop(s);
    let mut s = Store::open(&dir, RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(
        s.engine().unwrap().world().view().boot.unwrap().jobs[0].phase,
        Phase::Running
    );
    execute(&mut s, BootCommand::Advance { seconds: 30 });
    assert!(s
        .engine()
        .unwrap()
        .resources()
        .iter()
        .all(|r| r.occupied == 0));
    assert_eq!(
        serde_json::to_value(s.spatial_reference().unwrap()).unwrap(),
        space
    );
}
