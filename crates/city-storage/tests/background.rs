use mycity_core::{
    air::State,
    background::Command as B,
    exchange::Command as X,
    schedule::Phase,
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource, TxError},
};
use mycity_storage::{BackgroundPoint, CapacityPolicy, Error, Store};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const RULES: &str = "m0-storage-process-fixture-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-background-storage-{}-{}",
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
fn create(path: &Path) -> Store {
    create_mode(path, false)
}
fn create_mode(path: &Path, v2: bool) -> Store {
    let create = if v2 {
        Store::create_spatial_v2
    } else {
        Store::create_spatial
    };
    create(
        path,
        include_bytes!("../../../fixtures/m0-exchange-opening.json"),
        [
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ]
        .into_iter()
        .map(|(id, c)| Resource::new(id, c).unwrap())
        .collect(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}
fn envelope(s: &Store, a: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: match a {
            Action::Background {
                command: B::Accept { job, .. },
            } => mycity_core::background::accept_operation_id(job).unwrap(),
            _ => s.committed_sequence() + 1,
        },
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![a],
    }
}
fn apply(s: &mut Store, a: Action) -> Envelope {
    let cmd = envelope(s, a);
    let out = s
        .execute(Caller::System, cmd.clone())
        .unwrap_or_else(|error| {
            panic!(
                "action={:?}, hot={}, bg={:?}, timeline={:?}, error={error:?}",
                cmd.actions,
                s.engine().unwrap().journal().len(),
                s.background_reserve().unwrap(),
                s.engine().unwrap().timeline().unwrap().progress
            )
        });
    assert!(out.model.is_ok(), "{out:?}");
    assert!(out.has_persistent_receipt);
    cmd
}
fn bg(c: B) -> Action {
    Action::Background { command: c }
}
fn advance(limit: u16) -> Action {
    Action::Timeline {
        command: T::Advance { target: 30, limit },
    }
}
fn ready(path: &Path, freight: bool, limit: u16) -> Store {
    ready_from(create(path), freight, limit)
}
fn ready_from(mut s: Store, freight: bool, limit: u16) -> Store {
    if freight {
        apply(
            &mut s,
            Action::Exchange {
                command: X::Dispatch { quote: 1 },
            },
        );
    }
    apply(&mut s, Action::Timeline { command: T::Enable });
    apply(&mut s, bg(B::Enable));
    apply(&mut s, bg(B::Restart));
    for _ in 0..96 {
        apply(&mut s, advance(limit));
        if s.engine().unwrap().timeline().unwrap().progress.phase == Some(Phase::P4) {
            break;
        }
    }
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().progress.phase,
        Some(Phase::P4)
    );
    assert!(s.continuation_status().unwrap().unwrap().remaining_pages >= 1);
    apply(&mut s, bg(B::Lease { job: 1 }));
    s
}
fn finish(s: &Store) -> State {
    let mut state = s
        .engine()
        .unwrap()
        .background()
        .unwrap()
        .frozen_input()
        .unwrap();
    while !state.complete() {
        state.page(8192, 1024).unwrap();
    }
    state
}
fn stage(s: &mut Store) -> ([u8; 32], State) {
    let result = finish(s);
    let hash = s
        .import_background_candidate(1, 1, &result.checkpoint().unwrap())
        .unwrap();
    (hash, result)
}
fn asset(path: &Path, hash: &[u8; 32]) -> PathBuf {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for b in hash {
        write!(&mut text, "{b:02x}").unwrap();
    }
    path.join(format!("air-result-{text}.air"))
}
fn accept(s: &Store, hash: [u8; 32], generation: u32) -> Envelope {
    envelope(
        s,
        bg(B::Accept {
            job: 1,
            generation,
            candidate_sha256: hash,
        }),
    )
}
#[test]
fn empty_and_real_freight_large_pages_have_funded_barrier_and_restore_exactly_once() {
    for freight in [false, true] {
        for limit in [1, 256, 8192] {
            let t = Temp::new();
            let mut s = ready(&t.world(), freight, limit);
            let reservation = s.background_reserve().unwrap().unwrap();
            assert!(reservation.remaining_bytes() > 10 * 1024 * 1024);
            let (hash, result) = stage(&mut s);
            let staged = s.background_reserve().unwrap().unwrap();
            assert_eq!(staged.slots, reservation.slots - 1);
            let cmd = accept(&s, hash, 1);
            let before = s.committed_sequence();
            assert!(s
                .execute(Caller::System, cmd.clone())
                .unwrap()
                .model
                .is_ok());
            assert_eq!(
                s.engine().unwrap().background().unwrap().field(),
                Some(&result)
            );
            assert!(s.background_reserve().unwrap().is_none());
            assert!(s.continuation_status().unwrap().is_some());
            let model = s.state_hash().unwrap();
            drop(s);
            let mut s = Store::open(&t.world(), RULES).unwrap();
            assert_eq!(s.state_hash().unwrap(), model);
            assert_eq!(s.committed_sequence(), before + 1);
            assert!(s
                .execute(Caller::System, cmd.clone())
                .unwrap()
                .model
                .is_ok());
            assert_eq!(s.committed_sequence(), before + 1);
            while s.continuation_status().unwrap().is_some() {
                apply(&mut s, advance(limit));
            }
            assert!(s.continuation_status().unwrap().is_none());
            assert!(asset(&t.world(), &hash).exists());
            s.checkpoint().unwrap();
            assert!(!asset(&t.world(), &hash).exists());
            let complete = s.state_hash().unwrap();
            drop(s);
            let mut s = Store::open(&t.world(), RULES).unwrap();
            assert_eq!(s.state_hash().unwrap(), complete);
            assert!(s.execute(Caller::System, cmd).unwrap().model.is_ok());
            assert_eq!(s.state_hash().unwrap(), complete);
        }
    }
}
#[test]
fn staged_asset_survives_midflight_checkpoint_restart_and_generation_rebind() {
    let t = Temp::new();
    let mut s = ready(&t.world(), true, 256);
    let (hash, result) = stage(&mut s);
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    assert!(asset(&t.world(), &hash).exists());
    apply(&mut s, bg(B::Restart));
    apply(&mut s, bg(B::Lease { job: 1 }));
    assert!(s
        .import_background_candidate(1, 1, &result.checkpoint().unwrap())
        .is_err());
    let failed = accept(&s, hash, 1);
    assert_eq!(
        s.execute(Caller::System, failed).unwrap().model,
        Err(TxError::InvalidOperation)
    );
    assert_eq!(
        s.import_background_candidate(1, 2, &result.checkpoint().unwrap())
            .unwrap(),
        hash
    );
    let cmd = accept(&s, hash, 2);
    s.execute(Caller::System, cmd).unwrap().model.unwrap();
    assert_eq!(
        s.engine().unwrap().background().unwrap().field(),
        Some(&result)
    );
}
#[test]
fn committed_missing_or_corrupt_asset_is_rejected_before_tail_truncation_and_gc() {
    for missing in [false, true] {
        let t = Temp::new();
        let mut s = ready(&t.world(), true, 256);
        let (hash, _) = stage(&mut s);
        let cmd = accept(&s, hash, 1);
        s.execute(Caller::System, cmd).unwrap().model.unwrap();
        drop(s);
        let wal = t.world().join("wal.bin");
        OpenOptions::new()
            .append(true)
            .open(&wal)
            .unwrap()
            .write_all(b"uncommitted-tail")
            .unwrap();
        let length = fs::metadata(&wal).unwrap().len();
        let orphan = t.world().join(format!("air-result-{}.air", "0".repeat(64)));
        fs::write(&orphan, b"orphan").unwrap();
        if missing {
            fs::remove_file(asset(&t.world(), &hash)).unwrap();
        } else {
            fs::write(asset(&t.world(), &hash), b"corrupt").unwrap();
        }
        assert!(Store::open(&t.world(), RULES).is_err());
        assert_eq!(fs::metadata(wal).unwrap().len(), length);
        assert!(orphan.exists());
    }
}
#[test]
fn normal_disk_exhaustion_preserves_background_and_timeline_funding() {
    let t = Temp::new();
    let mut s = ready(&t.world(), true, 8192);
    let status = s.capacity_status().unwrap();
    let held = status.continuation_reserved_bytes + status.background_reserved_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: status.used_bytes + held + 256 * 1024,
        emergency_bytes: 8 * 1024 * 1024,
    })
    .unwrap();
    assert!(s.capacity_status().unwrap().normal_blocked);
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    s.prepare_worker_cache().unwrap();
    apply(&mut s, bg(B::Restart));
    let protected = s.background_reserve().unwrap();
    let extra_restart = envelope(&s, bg(B::Restart));
    assert!(matches!(
        s.execute(Caller::System, extra_restart),
        Err(Error::Capacity { .. })
    ));
    assert_eq!(s.background_reserve().unwrap(), protected);
    apply(&mut s, bg(B::Lease { job: 1 }));
    let result = finish(&s);
    let hash = s
        .import_background_candidate(1, 2, &result.checkpoint().unwrap())
        .unwrap();
    let cmd = accept(&s, hash, 2);
    s.execute(Caller::System, cmd).unwrap().model.unwrap();
    apply(&mut s, advance(8192));
    assert!(s.background_reserve().unwrap().is_none());
    assert!(s.continuation_status().unwrap().is_none());
    let next = envelope(
        &s,
        Action::Timeline {
            command: T::Advance {
                target: 60,
                limit: 8192,
            },
        },
    );
    let h = s.state_hash().unwrap();
    assert!(matches!(
        s.execute(Caller::System, next),
        Err(Error::Capacity { .. })
    ));
    assert_eq!(h, s.state_hash().unwrap());
}
#[test]
fn failed_operation_flood_cannot_consume_dedicated_control_or_receipt_slots() {
    let t = Temp::new();
    use mycity_core::{
        inbox::{Command as I, ExternalFact, ExternalReason},
        units::Money,
    };
    let mut s = create(&t.world());
    apply(&mut s, Action::Inbox { command: I::Enable });
    s.register_external_source().unwrap();
    s.reconcile_external_source(1).unwrap();
    apply(&mut s, Action::Timeline { command: T::Enable });
    apply(&mut s, bg(B::Enable));
    apply(&mut s, bg(B::Restart));
    let fact = |event, due| ExternalFact {
        source: 1,
        event,
        debtor: 4,
        creditor: 2,
        amount: Money::new(1).unwrap(),
        due,
        reason: ExternalReason::EmergencyRepair,
    };
    for event in 1..=12 {
        let f = fact(event, 0);
        s.queue_external(f).unwrap();
        s.receive_external(f).unwrap().model.unwrap();
        apply(
            &mut s,
            Action::Inbox {
                command: I::Settle {
                    sequence: u64::from(event),
                },
            },
        );
    }
    s.reconcile_external_source(13).unwrap();
    for _ in 0..16 {
        apply(&mut s, advance(8192));
        if s.engine()
            .unwrap()
            .background()
            .unwrap()
            .pending()
            .is_some()
        {
            break;
        }
    }
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().progress.phase,
        Some(Phase::P4)
    );
    apply(&mut s, bg(B::Lease { job: 1 }));
    let held = s.background_reserve().unwrap().unwrap();
    let mut failures = 0;
    for _ in 0..1100 {
        let mut cmd = envelope(&s, bg(B::Lease { job: 999 }));
        cmd.expected_revision = u64::MAX;
        match s.execute(Caller::System, cmd) {
            Ok(v) => {
                assert_eq!(v.model, Err(TxError::StaleRevision));
                failures += 1;
            }
            Err(Error::Invalid("background_receipt_reserve" | "timeline_receipt_reserve")) => break,
            other => panic!("{other:?}"),
        }
    }
    assert!(failures > 800);
    assert_eq!(s.background_reserve().unwrap().unwrap(), held);
    // Real source/cleanup work may consume the separate 64-slot management reserve.
    // The background result must still be able to spend its own slots afterwards.
    let pending = fact(13, 30);
    s.queue_external(pending).unwrap();
    assert!(matches!(
        s.receive_external(pending),
        Err(Error::FactBlocked(_) | Error::Invalid("background_receipt_reserve"))
    ));
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .exchange
            .unwrap()
            .inbox
            .facts
            .iter()
            .flatten()
            .count(),
        12
    );
    apply(
        &mut s,
        Action::Inbox {
            command: I::Handoff {
                sequence: 1,
                manager: 1,
            },
        },
    );
    s.receive_external(pending).unwrap().model.unwrap();
    for sequence in 2..=12 {
        apply(
            &mut s,
            Action::Inbox {
                command: I::Handoff {
                    sequence,
                    manager: 1,
                },
            },
        );
    }
    for sequence in 1..=12 {
        apply(
            &mut s,
            Action::Inbox {
                command: I::Archive { sequence },
            },
        );
    }
    let cleanup = 25;
    assert!(s.engine().unwrap().journal().len() > 960);
    assert_eq!(s.background_reserve().unwrap().unwrap(), held);
    let (hash, _) = stage(&mut s);
    let cmd = accept(&s, hash, 1);
    s.execute(Caller::System, cmd).unwrap().model.unwrap();
    let next = s.source_status().unwrap().next_event;
    s.reconcile_external_source(next).unwrap();
    apply(&mut s, advance(8192));
    assert!(s.continuation_status().unwrap().is_none());
    println!("BACKGROUND_RECEIPT_FLOOD rejected_requests={failures} actual_source_cleanup={cleanup}; background and clock reserve remain independently usable");
}
#[test]
fn actual_process_exit_at_every_import_and_accept_boundary_restores_original_result() {
    let import_points = [
        BackgroundPoint::Validated,
        BackgroundPoint::AssetHalf,
        BackgroundPoint::AssetSynced,
        BackgroundPoint::AssetPublished,
        BackgroundPoint::StagedRootPublished,
        BackgroundPoint::StagedCurrentRenamed,
        BackgroundPoint::StagedCurrentSynced,
    ];
    for point in import_points {
        let t = Temp::new();
        let s = ready(&t.world(), true, 8192);
        let result = finish(&s);
        let file = t.0.join("candidate.air");
        fs::write(&file, result.checkpoint().unwrap()).unwrap();
        let state = s.state_hash().unwrap();
        let seq = s.committed_sequence();
        drop(s);
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
            .args([
                "background-import",
                t.world().to_str().unwrap(),
                "1",
                "1",
                file.to_str().unwrap(),
                &format!("{point:?}"),
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(97),
            "{point:?} {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut s = Store::open(&t.world(), RULES).unwrap();
        assert_eq!(s.state_hash().unwrap(), state);
        assert_eq!(s.committed_sequence(), seq);
        let hash = s
            .import_background_candidate(1, 1, &result.checkpoint().unwrap())
            .unwrap();
        let cmd = accept(&s, hash, 1);
        s.execute(Caller::System, cmd).unwrap().model.unwrap();
        assert_eq!(
            s.engine().unwrap().background().unwrap().field(),
            Some(&result)
        );
    }
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
    for point in points {
        let t = Temp::new();
        let mut s = ready(&t.world(), true, 8192);
        let (hash, result) = stage(&mut s);
        let cmd = accept(&s, hash, 1);
        let file = t.0.join("accept.json");
        fs::write(&file, serde_json::to_vec(&cmd).unwrap()).unwrap();
        let seq = s.committed_sequence();
        drop(s);
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
            .args([
                "activity-commit",
                t.world().to_str().unwrap(),
                file.to_str().unwrap(),
                point,
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(97),
            "{point} {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut s = Store::open(&t.world(), RULES).unwrap();
        let out = s.execute(Caller::System, cmd).unwrap();
        assert!(out.model.is_ok());
        assert!(out.has_persistent_receipt);
        assert_eq!(s.committed_sequence(), seq + 1);
        assert_eq!(
            s.engine().unwrap().background().unwrap().field(),
            Some(&result)
        );
    }
    println!("BACKGROUND_ACTUAL_PROCESS_EXITS import=7 accept=11; original numerical state, ticket and receipt restored once");
}

#[test]
fn wrong_window_finite_forgery_and_old_job_cannot_consume_reservation_or_replace_field() {
    let t = Temp::new();
    let mut s = ready(&t.world(), true, 256);
    let result = finish(&s);
    let before = s.state_hash().unwrap();
    let reserve = s.background_reserve().unwrap();
    let mut v = serde_json::to_value(&result).unwrap();
    v["field"][0] = serde_json::json!(3f32.to_bits());
    let forged: State = serde_json::from_value(v).unwrap();
    assert!(s
        .import_background_candidate(1, 1, &forged.checkpoint().unwrap())
        .is_err());
    let mut input = result.input().clone();
    input.target = 60;
    let mut wrong = State::new(input).unwrap();
    while !wrong.complete() {
        wrong.page(8192, 1024).unwrap();
    }
    assert!(s
        .import_background_candidate(1, 1, &wrong.checkpoint().unwrap())
        .is_err());
    assert_eq!(before, s.state_hash().unwrap());
    assert_eq!(reserve, s.background_reserve().unwrap());
    let old_bytes = result.checkpoint().unwrap();
    let hash = s.import_background_candidate(1, 1, &old_bytes).unwrap();
    let cmd = accept(&s, hash, 1);
    s.execute(Caller::System, cmd).unwrap().model.unwrap();
    apply(&mut s, advance(256));
    apply(
        &mut s,
        Action::Timeline {
            command: T::Advance {
                target: 60,
                limit: 256,
            },
        },
    );
    apply(&mut s, bg(B::Lease { job: 2 }));
    let before = s.state_hash().unwrap();
    assert!(s.import_background_candidate(1, 1, &old_bytes).is_err());
    assert!(s.import_background_candidate(2, 1, &old_bytes).is_err());
    assert_eq!(s.state_hash().unwrap(), before);
}

#[test]
fn result_reference_limit_checkpoints_before_new_obligation_and_keeps_cold_receipts() {
    let t = Temp::new();
    let mut s = create(&t.world());
    apply(&mut s, Action::Timeline { command: T::Enable });
    apply(&mut s, bg(B::Enable));
    apply(&mut s, bg(B::Restart));
    let mut original = None;
    let mut peak = 0;
    for id in 1..=33 {
        let progress = s.engine().unwrap().timeline().unwrap().progress;
        let target = progress.target.unwrap_or(progress.last_complete + 30);
        apply(
            &mut s,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        apply(&mut s, bg(B::Lease { job: id }));
        let result = finish(&s);
        let hash = s
            .import_background_candidate(id, 1, &result.checkpoint().unwrap())
            .unwrap();
        let cmd = envelope(
            &s,
            bg(B::Accept {
                job: id,
                generation: 1,
                candidate_sha256: hash,
            }),
        );
        s.execute(Caller::System, cmd.clone())
            .unwrap()
            .model
            .unwrap();
        if id == 1 {
            original = Some(cmd);
        }
        apply(
            &mut s,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        let count = fs::read_dir(t.world())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("air-result-")
            })
            .count();
        peak = peak.max(count);
        assert!(count <= 32);
    }
    assert_eq!(peak, 32);
    assert_eq!(s.engine().unwrap().background().unwrap().version(), 33);
    let hash = s.state_hash().unwrap();
    assert!(
        s.execute(Caller::System, original.unwrap())
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(hash, s.state_hash().unwrap());
    drop(s);
    let s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(hash, s.state_hash().unwrap());
    println!("BACKGROUND_ASSET_ROTATION accepted=33 max_live_assets=32; checkpoint precedes next admission and cold receipt survives GC");
}

#[test]
fn worker_cache_charge_survives_missing_files_and_moves_out_of_task_reserve_once() {
    const CACHE: u64 = 6 * 1024 * 1024;
    const PAGE: u64 = 45120;
    let temp = Temp::new();
    let world = temp.world();
    let mut s = ready(&world, true, 256);
    let before = s.capacity_status().unwrap();
    let hash = s.state_hash().unwrap();
    let sequence = s.committed_sequence();
    let cache = s.prepare_worker_cache().unwrap();
    let after = s.capacity_status().unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), sequence);
    assert_eq!(after.worker_cache_charged_bytes, CACHE);
    assert_eq!(after.worker_cache_actual_bytes, 0);
    assert_eq!(
        before.background_reserved_bytes - after.background_reserved_bytes,
        CACHE + PAGE
    );
    let reserved = after.background_reserved_bytes;
    assert_eq!(s.prepare_worker_cache().unwrap(), cache);
    assert_eq!(
        s.capacity_status().unwrap().background_reserved_bytes,
        reserved
    );
    fs::create_dir(&cache).unwrap();
    fs::create_dir(cache.join("inputs")).unwrap();
    let f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(cache.join("inputs").join(".pending-123"))
        .unwrap();
    f.set_len(2 * 1024 * 1024).unwrap();
    assert_eq!(
        s.capacity_status().unwrap().worker_cache_actual_bytes,
        2 * 1024 * 1024
    );
    let charged = s.capacity_status().unwrap().used_bytes;
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(s.capacity_status().unwrap().used_bytes, charged);
    drop(s);
    let mut s = Store::open(&world, RULES).unwrap();
    assert_eq!(s.prepare_worker_cache().unwrap(), cache);
    assert_eq!(
        s.capacity_status().unwrap().background_reserved_bytes,
        reserved
    );
    let (digest, _) = stage(&mut s);
    let request = accept(&s, digest, 1);
    let result = s.execute(Caller::System, request.clone()).unwrap();
    assert!(result.model.is_ok());
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&world, RULES).unwrap();
    let replay = s.execute(Caller::System, request).unwrap();
    assert_eq!(replay.model, result.model);
    assert!(replay.has_persistent_receipt);
    assert_eq!(
        s.capacity_status().unwrap().worker_cache_charged_bytes,
        CACHE
    );
}

#[test]
fn canonical_accept_rejections_do_not_create_wal_or_cold_failed_receipts() {
    let temp = Temp::new();
    let world = temp.world();
    let mut s = ready(&world, true, 256);
    let (hash, _) = stage(&mut s);
    let valid = accept(&s, hash, 1);
    let mut stale = valid.clone();
    stale.expected_revision -= 1;
    let sequence = s.committed_sequence();
    let reserved = s.background_reserve().unwrap();
    let rejected = s.execute(Caller::System, stale).unwrap();
    assert!(rejected.model.is_err());
    assert!(!rejected.has_persistent_receipt);
    assert_eq!(s.committed_sequence(), sequence);
    assert_eq!(s.background_reserve().unwrap(), reserved);
    let mut ordinary_id = valid.clone();
    ordinary_id.operation_id = sequence + 1;
    let rejected = s.execute(Caller::System, ordinary_id).unwrap();
    assert_eq!(rejected.model, Err(TxError::InvalidOperation));
    assert!(!rejected.has_persistent_receipt);
    drop(s);
    let mut s = Store::open(&world, RULES).unwrap();
    assert_eq!(s.committed_sequence(), sequence);
    let receipt = s.execute(Caller::System, valid.clone()).unwrap();
    assert!(receipt.model.is_ok());
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&world, RULES).unwrap();
    let replay = s.execute(Caller::System, valid).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.model, receipt.model);
}

#[test]
fn v2_staged_asset_checkpoint_reopen_accept_and_gc_preserve_original_receipts() {
    let t = Temp::new();
    let mut s = ready_from(create_mode(&t.world(), true), true, 256);
    let (hash, result) = stage(&mut s);
    let reserved = s.background_reserve().unwrap();
    let continuation = s.continuation_status().unwrap();
    let sequence = s.committed_sequence();
    let old = s.receipt(1).unwrap().unwrap();
    s.checkpoint().unwrap();
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.background_reserve().unwrap(), reserved);
    assert_eq!(s.continuation_status().unwrap(), continuation);
    assert_eq!(s.committed_sequence(), sequence);
    assert!(asset(&t.world(), &hash).exists());
    let cmd = accept(&s, hash, 1);
    s.execute(Caller::System, cmd.clone())
        .unwrap()
        .model
        .unwrap();
    let original = s.receipt(cmd.operation_id).unwrap().unwrap();
    s.checkpoint().unwrap();
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(
        s.engine().unwrap().background().unwrap().field(),
        Some(&result)
    );
    assert!(!asset(&t.world(), &hash).exists());
    assert_eq!(
        serde_json::to_value(s.receipt(1).unwrap()).unwrap(),
        serde_json::to_value(Some(old)).unwrap()
    );
    assert_eq!(
        serde_json::to_value(s.receipt(cmd.operation_id).unwrap()).unwrap(),
        serde_json::to_value(Some(original)).unwrap()
    );
    let seq = s.committed_sequence();
    assert!(s.execute(Caller::System, cmd).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(), seq);
}
