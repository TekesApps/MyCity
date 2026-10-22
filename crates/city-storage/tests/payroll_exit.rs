use mycity_core::{
    timeline::{Command as T, Cycle, PayrollSpec},
    transaction::{Action, Caller, Envelope, TxError},
    units::Money,
};
use mycity_storage::{Store, WritePoint};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-payroll-exit-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("PAYROLL_EXIT_ARTIFACT_CREATED={}", path.display());
        Self(path)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("PAYROLL_EXIT_ARTIFACT_PRESERVED={}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn m(n: i64) -> Money {
    Money::new(n).unwrap()
}
fn create(path: &Path) -> Store {
    let opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    Store::create(path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES).unwrap()
}
fn envelope(s: &Store, command: T) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![Action::Timeline { command }],
    }
}
fn execute(s: &mut Store, command: T) {
    let request = envelope(s, command);
    let outcome = s.execute(Caller::System, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}
fn drive(s: &mut Store, target: u64) {
    drive_with_limit(s, target, 256);
}
fn drive_with_limit(s: &mut Store, target: u64, limit: u16) {
    for _ in 0..10_000 {
        execute(s, T::Advance { target, limit });
        if s.engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("Timeline did not complete target {target}");
}
fn cash(s: &Store, id: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == id)
        .unwrap()
        .cash
        .value()
}
fn payroll(s: &Store) -> mycity_core::timeline::Payroll {
    s.engine().unwrap().timeline().unwrap().payroll[0]
}
fn setup_with_paid_period_and_new_arrears(path: &Path) -> (Store, Envelope, i64, i64) {
    let mut s = create(path);
    execute(&mut s, T::Enable);
    execute(
        &mut s,
        T::AddPayroll {
            spec: PayrollSpec {
                id: 1,
                payer: 3,
                payee: 2,
                home_city: 0,
                hourly_cents: m(7),
                cycle: Cycle::Weekly,
                ends_at: 40 * 86400,
            },
        },
    );
    drive(&mut s, 7 * 86400);
    assert_eq!(payroll(&s).paid, m(392));
    drive(&mut s, 7 * 86400 + 10 * 3600);
    let c = payroll(&s);
    assert_eq!(c.accrued, m(399));
    assert_eq!(c.billed, m(392));
    assert_eq!(c.paid, m(392));
    let payer_cash = cash(&s, 3);
    let payee_cash = cash(&s, 2);
    s.checkpoint().unwrap();
    drop(s);

    let s = Store::open(path, RULES).unwrap();
    let end = envelope(&s, T::EndForExit { id: 1 });
    (s, end, payer_cash, payee_cash)
}

#[test]
fn payroll_exit_checkpoint_reopen_replay_and_explicit_settlement_conserve_cash() {
    let temp = Temp::new();
    let path = temp.world();
    let (mut s, end, payer_before, payee_before) = setup_with_paid_period_and_new_arrears(&path);
    let end_sequence = end.operation_id;
    let outcome = s.execute(Caller::System, end.clone()).unwrap();
    assert!(outcome.model.is_ok());
    assert!(outcome.has_persistent_receipt);
    assert_eq!(cash(&s, 3), payer_before);
    assert_eq!(cash(&s, 2), payee_before);
    let ended = payroll(&s);
    assert_eq!(ended.accrued, m(399));
    assert_eq!(ended.billed, m(399));
    assert_eq!(ended.paid, m(392));
    assert_eq!(ended.billed.checked_sub(ended.paid).unwrap(), m(7));
    assert!(!ended.working);
    assert_eq!(ended.next_due, None);
    assert_eq!(ended.spec.ends_at, 40 * 86400);

    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), end_sequence);
    let before_replay = s.state_hash().unwrap();
    let replay = s.execute(Caller::System, end.clone()).unwrap();
    assert!(replay.model.is_ok());
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.state_hash().unwrap(), before_replay);
    assert_eq!(payroll(&s), ended);
    assert_eq!(cash(&s, 3), payer_before);
    assert_eq!(cash(&s, 2), payee_before);

    execute(&mut s, T::SettlePayroll { id: 1 });
    let settled = payroll(&s);
    assert_eq!(settled.accrued, m(399));
    assert_eq!(settled.billed, m(399));
    assert_eq!(settled.paid, m(399));
    assert_eq!(cash(&s, 3), payer_before - 7);
    assert_eq!(cash(&s, 2), payee_before + 7);
    assert_eq!(cash(&s, 3) + cash(&s, 2), payer_before + payee_before);
    s.checkpoint().unwrap();
    drop(s);
    let s = Store::open(&path, RULES).unwrap();
    assert_eq!(payroll(&s).paid, m(399));
    assert_eq!(cash(&s, 3), payer_before - 7);
    assert_eq!(cash(&s, 2), payee_before + 7);
}

#[test]
fn payroll_exit_write_uncertainty_reopens_and_replays_exactly_once_before_settlement() {
    for point in [WritePoint::WalHeaderHalf, WritePoint::Applied] {
        let temp = Temp::new();
        let path = temp.world();
        let (mut s, end, payer_before, payee_before) =
            setup_with_paid_period_and_new_arrears(&path);
        let sequence_before = s.committed_sequence();
        assert!(s
            .execute_with_observer(Caller::System, end.clone(), |at| {
                if at == point {
                    Err(std::io::Error::other("injected lost payroll-exit response"))
                } else {
                    Ok(())
                }
            })
            .is_err());
        assert!(s.engine().is_err());
        drop(s);

        let mut s = Store::open(&path, RULES).unwrap();
        let committed_before_replay = point == WritePoint::Applied;
        assert_eq!(
            s.committed_sequence(),
            sequence_before + u64::from(committed_before_replay)
        );
        if committed_before_replay {
            assert_eq!(payroll(&s).billed, m(399));
        } else {
            assert_eq!(payroll(&s).billed, m(392));
        }
        let replay = s.execute(Caller::System, end.clone()).unwrap();
        assert!(replay.model.is_ok());
        assert!(replay.has_persistent_receipt);
        assert_eq!(s.committed_sequence(), sequence_before + 1);
        let after_replay = payroll(&s);
        assert_eq!(after_replay.accrued, m(399));
        assert_eq!(after_replay.billed, m(399));
        assert_eq!(after_replay.paid, m(392));
        assert_eq!(after_replay.spec.ends_at, 40 * 86400);
        assert_eq!(cash(&s, 3), payer_before);
        assert_eq!(cash(&s, 2), payee_before);

        let replay_hash = s.state_hash().unwrap();
        let retry = s.execute(Caller::System, end).unwrap();
        assert!(retry.has_persistent_receipt);
        assert_eq!(s.state_hash().unwrap(), replay_hash);
        execute(&mut s, T::SettlePayroll { id: 1 });
        assert_eq!(payroll(&s).paid, m(399));
        assert_eq!(cash(&s, 3), payer_before - 7);
        assert_eq!(cash(&s, 2), payee_before + 7);
        println!("PAYROLL_EXIT_WRITE_RECOVERY point={point:?} committed_before_retry={committed_before_replay}");
    }
}

#[test]
fn payroll_exit_rejects_incomplete_advance_then_non_system_caller_at_complete_time() {
    let temp = Temp::new();
    let mut s = create(&temp.world());
    execute(&mut s, T::Enable);
    execute(
        &mut s,
        T::AddPayroll {
            spec: PayrollSpec {
                id: 1,
                payer: 3,
                payee: 2,
                home_city: 0,
                hourly_cents: m(7),
                cycle: Cycle::Weekly,
                ends_at: 40 * 86400,
            },
        },
    );
    execute(
        &mut s,
        T::Advance {
            target: 3600,
            limit: 1,
        },
    );
    let progress = s.engine().unwrap().timeline().unwrap().progress;
    assert!(progress.target.is_some());
    let before = payroll(&s);
    let before_cash = (cash(&s, 3), cash(&s, 2));
    let request = envelope(&s, T::EndForExit { id: 1 });
    let rejected = s.execute(Caller::System, request).unwrap();
    assert_eq!(
        rejected.model,
        Err(TxError::Timeline(mycity_core::timeline::Error::Busy))
    );
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress, progress);
    assert_eq!(payroll(&s), before);
    assert_eq!((cash(&s, 3), cash(&s, 2)), before_cash);

    // Continue with the page budget reserved by the original in-flight request.
    drive_with_limit(&mut s, 3600, 1);
    let complete_progress = s.engine().unwrap().timeline().unwrap().progress;
    assert!(complete_progress.target.is_none());
    let complete_payroll = payroll(&s);

    let request = envelope(&s, T::EndForExit { id: 1 });
    let rejected = s.execute(Caller::Account(2), request).unwrap();
    assert_eq!(rejected.model, Err(TxError::Unauthorized));
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().progress,
        complete_progress
    );
    assert_eq!(payroll(&s), complete_payroll);
    assert_eq!((cash(&s, 3), cash(&s, 2)), before_cash);
}
