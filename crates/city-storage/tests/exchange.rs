use mycity_core::{
    boot::{Command as BootCommand, Facility},
    exchange::Command as ExchangeCommand,
    transaction::{Action, Caller, Envelope},
};
use mycity_storage::{Store, WritePoint};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-exchange-opening.json");
const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-exchange-store-{}-{}-{}",
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
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, c)| mycity_core::transaction::Resource::new(id, c).unwrap())
    .collect()
}
fn create(dir: &Path, input: &[u8]) -> Store {
    Store::create_spatial(
        dir,
        input,
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}
fn envelope(s: &Store, actions: Vec<Action>) -> Envelope {
    let e = s.engine().unwrap();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: e.world().view().revision,
        actions,
    }
}
fn execute(s: &mut Store, action: Action) {
    let cmd = envelope(s, vec![action]);
    let result = s.execute(Caller::System, cmd).unwrap().model;
    assert!(result.is_ok(), "{result:?}");
}
fn start(facility: Facility) -> Action {
    Action::Boot {
        command: BootCommand::Start {
            facility,
            reverse_request: true,
        },
    }
}
fn x(command: ExchangeCommand) -> Action {
    Action::Exchange { command }
}
fn dispatch(quote: u32) -> Action {
    x(ExchangeCommand::Dispatch { quote })
}
fn advance(s: &mut Store, seconds: u32) {
    execute(s, Action::RegionAdvance { seconds });
}
fn prepare(s: &mut Store) {
    execute(s, start(Facility::Pump));
    advance(s, 60);
    execute(s, start(Facility::Generator));
    advance(s, 60);
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
fn regional_fulfilment_recovers_joint_old_or_new_state_at_121_actual_process_exits() {
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
    for stage in [
        "dispatch_freight",
        "arrival_freight",
        "dispatch_electric",
        "arrival_electric",
        "dispatch_water",
        "arrival_water",
        "closed_job_completion",
        "joint_fault",
        "link_failure",
        "link_repair",
        "map_switch",
    ] {
        let base = t.0.join(stage);
        let mut s = create(&base, INPUT);
        let action = match stage {
            "dispatch_freight" => dispatch(1),
            "arrival_freight" => {
                execute(&mut s, dispatch(1));
                advance(&mut s, 119);
                Action::RegionAdvance { seconds: 1 }
            }
            "dispatch_electric" => {
                prepare(&mut s);
                dispatch(6)
            }
            "arrival_electric" => {
                prepare(&mut s);
                execute(&mut s, dispatch(6));
                advance(&mut s, 61);
                Action::RegionAdvance { seconds: 1 }
            }
            "dispatch_water" => {
                prepare(&mut s);
                dispatch(7)
            }
            "arrival_water" => {
                prepare(&mut s);
                execute(&mut s, dispatch(7));
                advance(&mut s, 1083);
                Action::RegionAdvance { seconds: 1 }
            }
            "closed_job_completion" => {
                execute(&mut s, start(Facility::Pump));
                execute(&mut s, Action::RegionActivate { city: 2 });
                advance(&mut s, 59);
                Action::RegionAdvance { seconds: 1 }
            }
            "joint_fault" => {
                execute(&mut s, start(Facility::Pump));
                execute(&mut s, Action::RegionActivate { city: 2 });
                advance(&mut s, 30);
                Action::Boot {
                    command: BootCommand::Fault {
                        facility: Facility::Pump,
                    },
                }
            }
            "link_failure" => {
                execute(&mut s, dispatch(1));
                advance(&mut s, 30);
                x(ExchangeCommand::SetLink {
                    edge: 2,
                    open: false,
                })
            }
            "link_repair" => {
                execute(&mut s, dispatch(1));
                advance(&mut s, 30);
                execute(
                    &mut s,
                    x(ExchangeCommand::SetLink {
                        edge: 2,
                        open: false,
                    }),
                );
                advance(&mut s, 200);
                x(ExchangeCommand::SetLink {
                    edge: 2,
                    open: true,
                })
            }
            "map_switch" => {
                execute(&mut s, dispatch(1));
                advance(&mut s, 30);
                Action::RegionActivate { city: 3 }
            }
            _ => unreachable!(),
        };
        let cmd = envelope(&s, vec![action]);
        let old = s.state_hash().unwrap();
        let seq = s.committed_sequence();
        drop(s);
        let file = t.0.join(format!("{stage}.json"));
        fs::write(&file, serde_json::to_vec(&cmd).unwrap()).unwrap();
        let reference = t.0.join(format!("ref-{stage}"));
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
                .arg("activity-commit")
                .arg(&dir)
                .arg(&file)
                .arg(point)
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(97),
                "{stage}/{point}:{}",
                String::from_utf8_lossy(&out.stderr)
            );
            exits += 1;
            let mut s = Store::open(&dir, RULES).unwrap();
            let committed = matches!(
                point,
                "CurrentRenamed" | "CurrentSynced" | "Applied" | "Responded"
            );
            assert_eq!(
                s.state_hash().unwrap(),
                if committed { new.clone() } else { old.clone() }
            );
            assert_eq!(s.committed_sequence(), seq + u64::from(committed));
            assert!(s
                .execute(Caller::System, cmd.clone())
                .unwrap()
                .model
                .is_ok());
            assert_eq!(s.state_hash().unwrap(), new);
            assert_eq!(s.committed_sequence(), seq + 1);
            if stage == "arrival_freight" {
                let world = s.engine().unwrap().world().view();
                let quote = world.exchange.unwrap().quotes[0];
                assert_eq!(quote.source_fragment.unwrap().lot_id, 11);
                assert_eq!(quote.source_fragment.unwrap().offset.value(), 0);
                assert_eq!(quote.source_fragment.unwrap().basis.value(), 400);
            }
        }
    }
    assert_eq!(exits, 121);
    println!("actual_exchange_process_exits=121; joint_ledger_clock_capacity_payment_spatial_root_and_exactly_once_retry_verified");
}
#[test]
fn uncertain_arrival_settlement_hides_writer_and_recovers_once_with_provenance() {
    let t = Temp::new();
    let dir = t.0.join("uncertain");
    let mut s = create(&dir, INPUT);
    execute(&mut s, dispatch(1));
    advance(&mut s, 119);
    let cmd = envelope(&s, vec![Action::RegionAdvance { seconds: 1 }]);
    assert!(s
        .execute_with_observer(Caller::System, cmd.clone(), |p| {
            if p == WritePoint::CurrentRenamed {
                Err(std::io::Error::other("uncertain arrival"))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(s.engine().is_err());
    drop(s);
    let mut s = Store::open(&dir, RULES).unwrap();
    let hash = s.state_hash().unwrap();
    let result = s.execute(Caller::System, cmd).unwrap().model;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(s.state_hash().unwrap(), hash);
    let w = s.engine().unwrap().world().view();
    assert_eq!(
        w.stocks
            .iter()
            .find(|s| s.id == 8)
            .unwrap()
            .quantity
            .value(),
        1000
    );
    assert!(w.exchange.unwrap().quotes[0].source_fragment.is_some());
}
#[test]
fn regional_store_requires_bound_spatial_frames_and_flat_template_before_creating_directory() {
    let t = Temp::new();
    let dir = t.0.join("missing");
    assert!(Store::create(&dir, INPUT, resources(), RULES).is_err());
    assert!(!dir.exists());
    let dir = t.0.join("wrong");
    assert!(Store::create_spatial(
        &dir,
        INPUT,
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png")
    )
    .is_err());
    assert!(!dir.exists());
}

#[test]
fn full_checkpoints_keep_running_failed_and_paused_physical_state() {
    for scenario in 0..3 {
        let t = Temp::new();
        let mut s = create(&t.0.join("world"), INPUT);
        if scenario < 2 {
            execute(&mut s, start(Facility::Pump));
            if scenario == 1 {
                execute(
                    &mut s,
                    Action::Boot {
                        command: BootCommand::Fault {
                            facility: Facility::Pump,
                        },
                    },
                );
            }
        } else {
            prepare(&mut s);
            execute(&mut s, dispatch(1));
            execute(
                &mut s,
                x(ExchangeCommand::SetLink {
                    edge: 1,
                    open: false,
                }),
            );
        }
        let hash = s.state_hash().unwrap();
        let view = serde_json::to_value(s.engine().unwrap().world().view()).unwrap();
        s.checkpoint().unwrap();
        s.checkpoint().unwrap();
        drop(s);
        let mut s = Store::open(&t.0.join("world"), RULES).unwrap();
        assert_eq!(s.state_hash().unwrap(), hash);
        assert_eq!(
            serde_json::to_value(s.engine().unwrap().world().view()).unwrap(),
            view
        );
        if scenario == 0 {
            advance(&mut s, 60);
        } else if scenario == 1 {
            execute(
                &mut s,
                Action::Boot {
                    command: BootCommand::ClearFailure {
                        facility: Facility::Pump,
                    },
                },
            );
        } else {
            execute(
                &mut s,
                x(ExchangeCommand::SetLink {
                    edge: 1,
                    open: true,
                }),
            );
            advance(&mut s, 120);
        }
        s.checkpoint().unwrap();
        drop(s);
        Store::open(&t.0.join("world"), RULES).unwrap();
    }
}
