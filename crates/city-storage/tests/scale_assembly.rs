use mycity_core::{
    transaction::{Action, Caller, Envelope, Resource},
    units::Quantity,
};
use mycity_storage::{Store, WritePoint};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-t13a-full-opening-candidate.json");
const RULES: &str = "t13a-scale-assembly-v1";
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mycity-scale-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn create(path: &Path) -> Store {
    let resources = [
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
    .map(|(id, cap)| Resource::new(id, cap).unwrap())
    .collect();
    Store::create_spatial(
        path,
        INPUT,
        resources,
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}
fn movement(s: &Store) -> Envelope {
    let e = s.engine().unwrap();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: 1,
        expected_revision: e.world().view().revision,
        actions: vec![Action::MoveStock {
            from: e.world().stock_ref(15).unwrap(),
            to: e.world().stock_ref(16).unwrap(),
            quantity: Quantity::new(500).unwrap(),
        }],
    }
}
#[test]
fn nonzero_extra_stock_indices_lots_and_original_receipts_survive_wal_and_cold_checkpoint_reopen() {
    let t = Temp::new();
    let path = t.0.join("world");
    let mut s = create(&path);
    let cmd = movement(&s);
    let receipt = s.execute(Caller::System, cmd.clone()).unwrap();
    assert!(receipt.model.is_ok());
    let original_record = serde_json::to_vec(&s.receipt(1).unwrap().unwrap()).unwrap();
    let expected = s.engine().unwrap().world().view();
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.engine().unwrap().world().view(), expected);
    let retry = s.execute(Caller::System, cmd.clone()).unwrap();
    assert!(retry.has_persistent_receipt);
    assert_eq!(retry.model, receipt.model);
    assert_eq!(s.committed_sequence(), 1);
    assert_eq!(
        serde_json::to_vec(&s.receipt(1).unwrap().unwrap()).unwrap(),
        original_record
    );
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.engine().unwrap().world().view(), expected);
    let retry = s.execute(Caller::System, cmd).unwrap();
    assert!(retry.has_persistent_receipt);
    assert_eq!(retry.model, receipt.model);
    assert_eq!(s.committed_sequence(), 1);
    assert_eq!(
        serde_json::to_vec(&s.receipt(1).unwrap().unwrap()).unwrap(),
        original_record
    );
}
#[test]
fn actual_process_exits_keep_scaled_extra_stock_old_or_new_and_retry_once() {
    if let Ok(path) = std::env::var("MYCITY_SCALE_EXIT_WORLD") {
        let point = std::env::var("MYCITY_SCALE_EXIT_POINT").unwrap();
        let mut s = Store::open(Path::new(&path), RULES).unwrap();
        let cmd = movement(&s);
        s.execute_with_observer(Caller::System, cmd, |at| {
            if format!("{at:?}") == point {
                std::process::exit(73);
            }
            Ok(())
        })
        .unwrap();
        if point == "Responded" {
            std::process::exit(73);
        }
        panic!("exit point missing");
    }
    let t = Temp::new();
    let points = [
        WritePoint::Prepared,
        WritePoint::WalPayloadHalf,
        WritePoint::WalSynced,
        WritePoint::RootPublished,
        WritePoint::CurrentRenamed,
        WritePoint::CurrentSynced,
        WritePoint::Applied,
    ];
    for point in points {
        let label = format!("{point:?}");
        let path = t.0.join(&label);
        let s = create(&path);
        let command = movement(&s);
        let original = s.engine().unwrap().world().view();
        drop(s);
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "actual_process_exits_keep_scaled_extra_stock_old_or_new_and_retry_once",
                "--nocapture",
            ])
            .env("MYCITY_SCALE_EXIT_WORLD", &path)
            .env("MYCITY_SCALE_EXIT_POINT", &label)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73), "{label}");
        let mut s = Store::open(&path, RULES).unwrap();
        let before = s.engine().unwrap().world().view();
        if before.revision == original.revision {
            assert_eq!(before, original);
        } else {
            assert_eq!(
                (
                    before.stocks[14].quantity.value(),
                    before.stocks[14].basis.value()
                ),
                (500, 150)
            );
            assert_eq!(
                (
                    before.stocks[15].quantity.value(),
                    before.stocks[15].basis.value()
                ),
                (500, 150)
            );
        }
        let first = s.execute(Caller::System, command.clone()).unwrap();
        assert!(first.model.is_ok());
        let after = s.engine().unwrap().world().view();
        let second = s.execute(Caller::System, command).unwrap();
        assert!(second.has_persistent_receipt);
        assert_eq!(second.model, first.model);
        assert_eq!(s.committed_sequence(), 1);
        assert_eq!(s.engine().unwrap().world().view(), after);
        assert_eq!(
            after.stocks[14].quantity.value() + after.stocks[15].quantity.value(),
            1000
        );
        assert_eq!(
            after.stocks[14].basis.value() + after.stocks[15].basis.value(),
            300
        );
    }
    println!("T13A_SCALE actual_process_exits=7; nonzero quantity/basis/lot/owner/index recovered old-or-new and once-only retry");
}
