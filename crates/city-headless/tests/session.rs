use mycity_core::{
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const RULES: &str = env!("MYCITY_SOURCE_SHA256");
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-session-{}-{}",
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
    Store::create(
        p,
        include_bytes!("../../../fixtures/m0-region-opening.json"),
        vec![],
        RULES,
    )
    .unwrap()
}
fn env(op: u64, rev: u64, seconds: u32) -> Value {
    json!(Envelope {
        world: 1,
        branch: 1,
        operation_id: op,
        expected_revision: rev,
        actions: vec![Action::RegionAdvance { seconds }]
    })
}
fn request(id: u64, request: Value) -> Value {
    json!({"id":id,"request":request})
}
fn session(p: &Path, identity: [&str; 2], requests: Vec<Value>) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args([
            "session",
            p.to_str().unwrap(),
            RULES,
            identity[0],
            identity[1],
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut pipe = child.stdin.take().unwrap();
        for q in requests {
            writeln!(pipe, "{q}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
#[test]
fn pause_step_run_and_cold_receipt_are_exact_across_real_reopen() {
    let t = Temp::new();
    drop(create(&t.world()));
    let e = env(100, 0, 30);
    let a = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"status"})),
            request(2, json!({"command":"run","envelope":e})),
            request(3, json!({"command":"step","envelope":e})),
            request(4, json!({"command":"pause"})),
            request(5, json!({"command":"receipt","operation_id":100})),
            request(6, json!({"command":"receipt","operation_id":999})),
        ],
    );
    assert_eq!(a[0]["result"]["paused"], true);
    assert_eq!(a[1]["error"]["code"], "pause_state");
    assert!(a[2]["result"]["outcome"]["model"]["Ok"].is_object());
    assert_eq!(a[3]["result"]["committed_game_seconds"], 30);
    assert_eq!(a[4]["result"]["state"], "recorded");
    assert_eq!(a[5]["result"]["state"], "unknown");
    let mut s = Store::open(&t.world(), RULES).unwrap();
    let hash = s.state_hash().unwrap();
    s.checkpoint().unwrap();
    drop(s);
    let b = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(7, json!({"command":"step","envelope":e})),
            request(8, json!({"command":"receipt","operation_id":100})),
            request(9, json!({"command":"status"})),
        ],
    );
    assert_eq!(b[0]["result"]["already_recorded"], true);
    assert_eq!(b[1]["result"]["record"], a[4]["result"]["record"]);
    assert_eq!(b[2]["result"]["paused"], true);
    assert_eq!(b[2]["result"]["state_sha256"], hash);
}
#[test]
fn json_cannot_elevate_identity_and_failed_transactions_remain_queryable() {
    let t = Temp::new();
    let s = create(&t.world());
    let w = s.engine().unwrap().world();
    let transfer = |op, amount| {
        json!(Envelope {
            world: 1,
            branch: 1,
            operation_id: op,
            expected_revision: 0,
            actions: vec![Action::Transfer {
                from: w.account_ref(3).unwrap(),
                to: w.account_ref(2).unwrap(),
                amount: Money::new(amount).unwrap()
            }]
        })
    };
    let denied = transfer(1, 100);
    let lack = transfer(2, 9_000_000);
    drop(s);
    let out = session(
        &t.world(),
        ["account", "2"],
        vec![
            json!({"id":1,"caller":"system","request":{"command":"submit","envelope":denied}}),
            request(2, json!({"command":"submit","envelope":denied})),
            request(3, json!({"command":"step","envelope":env(3,0,10)})),
        ],
    );
    assert_eq!(out[0]["error"]["code"], "invalid_request");
    assert_eq!(out[1]["result"]["outcome"]["model"]["Err"], "unauthorized");
    assert_eq!(out[2]["result"]["outcome"]["model"]["Err"], "unauthorized");
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(4, json!({"command":"submit","envelope":lack})),
            request(5, json!({"command":"receipt","operation_id":2})),
        ],
    );
    assert_eq!(
        out[0]["result"]["outcome"]["model"]["Err"],
        "insufficient_funds"
    );
    assert!(out[1]["result"]["record"].is_object());
}
#[test]
fn four_real_paced_speeds_preserve_business_result_without_claiming_throughput() {
    let mut views = vec![];
    for day in [600000, 60000, 10000, 1000] {
        let t = Temp::new();
        drop(create(&t.world()));
        let out = session(
            &t.world(),
            ["operator", "system"],
            vec![
                request(1, json!({"command":"speed","day_wall_ms":day})),
                request(2, json!({"command":"resume"})),
                request(3, json!({"command":"run","envelope":env(1,0,1)})),
                request(4, json!({"command":"pause"})),
            ],
        );
        let v = &out[2]["result"];
        assert!(v["outcome"]["model"]["Ok"].is_object(), "{out:?}");
        assert!(v["elapsed_ns"].as_u64().unwrap() >= v["pacing_wait_ns"].as_u64().unwrap());
        assert!(v["actual_game_seconds_per_wall_second"].as_f64().unwrap() > 0.0);
        assert_eq!(out[3]["result"]["committed_game_seconds"], 1);
        views.push(
            serde_json::to_value(
                Store::open(&t.world(), RULES)
                    .unwrap()
                    .engine()
                    .unwrap()
                    .world()
                    .view(),
            )
            .unwrap(),
        );
    }
    assert!(views.windows(2).all(|w| w[0] == w[1]));
}
#[test]
fn slicing_across_due_boundaries_changes_no_cash_obligation_or_clock() {
    let t = Temp::new();
    drop(create(&t.world()));
    session(
        &t.world(),
        ["operator", "system"],
        vec![request(1, json!({"command":"step","envelope":env(1,0,30)}))],
    );
    let a = Store::open(&t.world(), RULES)
        .unwrap()
        .engine()
        .unwrap()
        .world()
        .view();
    let t2 = Temp::new();
    drop(create(&t2.world()));
    session(
        &t2.world(),
        ["operator", "system"],
        (0..3)
            .map(|i| request(i, json!({"command":"step","envelope":env(i+1,i,10)})))
            .collect(),
    );
    let b = Store::open(&t2.world(), RULES)
        .unwrap()
        .engine()
        .unwrap()
        .world()
        .view();
    assert_eq!(a.accounts, b.accounts);
    assert_eq!(a.stocks, b.stocks);
    assert_eq!(a.region, b.region);
}
#[test]
fn command_conflicts_scope_revision_and_paging_do_not_change_business_state() {
    let t = Temp::new();
    drop(create(&t.world()));
    let mut wrong = env(2, 1, 1);
    wrong["branch"] = json!(2);
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"step","envelope":env(1,0,1)})),
            request(2, json!({"command":"step","envelope":env(1,0,2)})),
            request(3, json!({"command":"step","envelope":wrong})),
            request(4, json!({"command":"step","envelope":env(3,0,1)})),
            request(
                5,
                json!({"command":"query","section":"receipts","limit":0,"cursor":null}),
            ),
            request(6, json!({"command":"submit","envelope":env(4,1,1)})),
            request(7, json!({"command":"status"})),
        ],
    );
    assert_eq!(
        out[1]["result"]["outcome"]["model"]["Err"],
        "operation_conflict"
    );
    assert_eq!(out[2]["result"]["outcome"]["model"]["Err"], "wrong_scope");
    assert_eq!(
        out[3]["result"]["outcome"]["model"]["Err"],
        "stale_revision"
    );
    assert_eq!(out[4]["error"]["reason"], "query_page_limit");
    assert_eq!(out[5]["error"]["code"], "clock_command_shape");
    assert_eq!(out[6]["result"]["committed_game_seconds"], 1);
}
#[test]
fn malformed_and_oversized_input_are_bounded_and_do_not_commit() {
    let t = Temp::new();
    drop(create(&t.world()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args([
            "session",
            t.world().to_str().unwrap(),
            RULES,
            "operator",
            "system",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b'x'; 32769])
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("session_line_limit"));
    assert_eq!(
        Store::open(&t.world(), RULES).unwrap().committed_sequence(),
        0
    );
}

#[test]
fn huge_time_speed_and_stale_cursor_reject_without_mutation() {
    let t = Temp::new();
    let mut s = create(&t.world());
    for op in 1..=2 {
        s.execute(
            mycity_core::transaction::Caller::System,
            serde_json::from_value(env(op, op - 1, 1)).unwrap(),
        )
        .unwrap();
    }
    let page = s.query(mycity_storage::Query::Receipts, None, 1).unwrap();
    let cursor = page.next_cursor.unwrap();
    drop(s);
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"speed","day_wall_ms":123})),
            request(2, json!({"command":"resume"})),
            request(3, json!({"command":"run","envelope":env(3,2,u32::MAX)})),
            request(4, json!({"command":"pause"})),
            request(5, json!({"command":"step","envelope":env(3,2,1)})),
            request(
                6,
                json!({"command":"query","section":"receipts","limit":1,"cursor":cursor}),
            ),
            request(7, json!({"command":"status"})),
        ],
    );
    assert_eq!(out[0]["error"]["code"], "unsupported_speed");
    assert_eq!(out[2]["error"]["code"], "wall_budget");
    assert_eq!(out[5]["error"]["reason"], "query_snapshot_changed");
    assert_eq!(out[6]["result"]["committed_game_seconds"], 3);
}
#[test]
fn disk_backpressure_cannot_be_bypassed_by_step_or_speed() {
    let t = Temp::new();
    let mut s = create(&t.world());
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(mycity_storage::CapacityPolicy {
        total_bytes: used + 8192,
        emergency_bytes: 4096,
    })
    .unwrap();
    // Fill actual normal+emergency disk quota; no permission is gained by a CLI control.
    fs::write(t.world().join("finite-budget-test.bin"), vec![0u8; 8192]).unwrap();
    drop(s);
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"speed","day_wall_ms":1000})),
            request(2, json!({"command":"step","envelope":env(1,0,1)})),
            request(3, json!({"command":"status"})),
        ],
    );
    assert_eq!(out[1]["error"]["code"], "storage_capacity");
    assert_eq!(out[2]["result"]["paused"], true);
    assert_eq!(out[2]["result"]["committed_game_seconds"], 0);
}
#[test]
fn source_reconciliation_blocks_clock_but_preserves_observation() {
    use mycity_core::transaction::{Caller, Resource};
    let t = Temp::new();
    let mut s = Store::create_spatial(
        &t.world(),
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
        .map(|(id, n)| Resource::new(id, n).unwrap())
        .collect(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap();
    s.execute(
        Caller::System,
        Envelope {
            world: 1,
            branch: 1,
            operation_id: 1,
            expected_revision: 0,
            actions: vec![Action::Inbox {
                command: mycity_core::inbox::Command::Enable,
            }],
        },
    )
    .unwrap();
    s.register_external_source().unwrap();
    let hash = s.state_hash().unwrap();
    drop(s);
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"step","envelope":env(2,1,1)})),
            request(
                2,
                json!({"command":"query","section":"blocked","limit":16,"cursor":null}),
            ),
            request(3, json!({"command":"status"})),
        ],
    );
    assert_eq!(out[0]["error"]["code"], "source_sync_required");
    assert!(out[1]["result"]["rows"].is_array());
    assert_eq!(out[2]["result"]["state_sha256"], hash);
    assert_eq!(out[2]["result"]["paused"], true);
}

#[test]
fn independent_boot_profile_completes_real_pump_before_pause_confirmation() {
    use mycity_core::transaction::Resource;
    let t = Temp::new();
    drop(
        Store::create(
            &t.world(),
            include_bytes!("../../../fixtures/m0-boot-opening.json"),
            vec![
                Resource::new(101, 1).unwrap(),
                Resource::new(102, 1).unwrap(),
            ],
            RULES,
        )
        .unwrap(),
    );
    let start = json!({"world":1,"branch":1,"operation_id":1,"expected_revision":0,"actions":[{"kind":"boot","command":{"kind":"start","facility":"pump","reverse_request":false}}]});
    let advance = json!({"world":1,"branch":1,"operation_id":2,"expected_revision":1,"actions":[{"kind":"boot","command":{"kind":"advance","seconds":60}}]});
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"submit","envelope":start})),
            request(2, json!({"command":"step","envelope":advance})),
            request(3, json!({"command":"pause"})),
        ],
    );
    assert!(
        out[0]["result"]["outcome"]["model"]["Ok"].is_object(),
        "{out:?}"
    );
    assert_eq!(out[2]["result"]["committed_game_seconds"], 60);
    let s = Store::open(&t.world(), RULES).unwrap();
    assert!(s.engine().unwrap().leases().iter().all(Option::is_none));
}

#[test]
fn paced_economy_day_matches_unpaced_committed_business_and_reports_real_wall_time() {
    use mycity_core::transaction::Caller;
    let t = Temp::new();
    drop(
        Store::create(
            &t.world(),
            include_bytes!("../../../fixtures/m0-economy-opening.json"),
            vec![],
            RULES,
        )
        .unwrap(),
    );
    let mut requests = vec![request(0, json!({"command":"resume"}))];
    for i in 0..24 {
        requests.push(request(i+1,json!({"command":"run","envelope":{"world":1,"branch":1,"operation_id":i+1,"expected_revision":i,"actions":[{"kind":"economy","command":{"kind":"advance","seconds":3600}}]}})));
    }
    requests.push(request(25, json!({"command":"pause"})));
    let started = std::time::Instant::now();
    let out = session(&t.world(), ["operator", "system"], requests);
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(
        out[25]["result"]["committed_game_seconds"], 86400,
        "{out:?}"
    );
    assert_eq!(out[25]["result"]["paused"], true);
    // Twenty-four releases start at 0, 2.5, ..., 57.5 seconds. Model/IPC
    // consumes those periods; do not require an extra sleep after the last job.
    assert!(elapsed >= 57.5);
    assert_eq!(out[1]["result"]["pacing_wait_ns"], 0);
    assert!(out[1..25]
        .iter()
        .all(|r| r["result"]["pacing_interval_ns"] == 2_500_000_000u64));
    assert!(out[1..25]
        .iter()
        .all(|r| r["result"]["outcome"]["has_persistent_receipt"] == true));
    let t2 = Temp::new();
    let mut direct = Store::create(
        &t2.world(),
        include_bytes!("../../../fixtures/m0-economy-opening.json"),
        vec![],
        RULES,
    )
    .unwrap();
    for i in 0..24 {
        let e:Envelope=serde_json::from_value(json!({"world":1,"branch":1,"operation_id":i+1,"expected_revision":i,"actions":[{"kind":"economy","command":{"kind":"advance","seconds":3600}}]})).unwrap();
        assert!(direct.execute(Caller::System, e).unwrap().model.is_ok());
    }
    let paced = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(paced.state_hash().unwrap(), direct.state_hash().unwrap());
    println!(
        "T10_PACED_DAY {}",
        json!({"game_seconds":86400,"target_wall_seconds":60,"actual_wall_seconds":elapsed,"actual_game_seconds_per_wall_second":86400.0/elapsed,"target_exactly_met":elapsed<=60.0,"persistent_commits":24,"same_state_as_unpaced":true,"state_sha256":paced.state_hash().unwrap(),"source_sha256":out[25]["result"]["provenance"]["source_sha256"],"store_owned_capacity_bytes":paced.owned_capacity_bytes().unwrap(),"memory_16gb_gate":"not_run"})
    );
}

#[test]
fn old_source_identity_cannot_be_used_to_write_with_new_binary() {
    let t = Temp::new();
    drop(create(&t.world()));
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args([
            "session",
            t.world().to_str().unwrap(),
            "old-code-identity",
            "operator",
            "system",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("rules_identity_must_match_current_source")
    );
    assert_eq!(
        Store::open(&t.world(), RULES).unwrap().committed_sequence(),
        0
    );
}

#[test]
fn timeline_partial_status_query_reopen_and_exact_receipt() {
    use mycity_core::timeline::Command as T;
    let t = Temp::new();
    let mut s = create(&t.world());
    for (i, c) in [
        T::Enable,
        T::Claims {
            first_id: 1,
            count: 1,
            payer: 3,
            payee: 2,
            amount: Money::new(1).unwrap(),
            due: 3600,
        },
        T::Advance {
            target: 3600,
            limit: 1,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let e = Envelope {
            world: 1,
            branch: 1,
            operation_id: i as u64 + 1,
            expected_revision: i as u64,
            actions: vec![Action::Timeline { command: c }],
        };
        s.execute(Caller::System, e).unwrap().model.unwrap();
    }
    let hash = s.state_hash().unwrap();
    let ticket = s.continuation_status().unwrap();
    drop(s);
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"status"})),
            request(
                2,
                json!({"command":"query","section":"timeline","limit":8,"cursor":null}),
            ),
            request(3, json!({"command":"receipt","operation_id":3})),
        ],
    );
    assert_eq!(out[0]["result"]["committed_game_seconds"], 0, "{out:?}");
    assert_eq!(out[0]["result"]["evidence"], "processing", "{out:?}");
    assert_eq!(out[1]["result"]["complete_boundary"], false, "{out:?}");
    let s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.continuation_status().unwrap(), ticket);
    assert!(out[2]["result"].is_object(), "{out:?}");
    let mut s = s;
    for id in 4..200 {
        let e = Envelope {
            world: 1,
            branch: 1,
            operation_id: id,
            expected_revision: s.engine().unwrap().world().view().revision,
            actions: vec![Action::Timeline {
                command: T::Advance {
                    target: 3600,
                    limit: 1,
                },
            }],
        };
        s.execute(Caller::System, e).unwrap().model.unwrap();
        if s.engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            break;
        }
    }
    assert_eq!(
        s.engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete,
        3600
    );
    let later = Envelope {
        world: 1,
        branch: 1,
        operation_id: 201,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![Action::Timeline {
            command: T::Advance {
                target: 7200,
                limit: 256,
            },
        }],
    };
    s.execute(Caller::System, later).unwrap().model.unwrap();
    assert_eq!(
        s.engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete,
        7200
    );
    let h = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    drop(s);
    let old = Envelope {
        world: 1,
        branch: 1,
        operation_id: 3,
        expected_revision: 2,
        actions: vec![Action::Timeline {
            command: T::Advance {
                target: 3600,
                limit: 1,
            },
        }],
    };
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![request(4, json!({"command":"step","envelope":old}))],
    );
    assert_eq!(
        out[0]["result"]["outcome"]["has_persistent_receipt"], true,
        "{out:?}"
    );
    let s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), h);
    assert_eq!(s.committed_sequence(), seq);
}

#[test]
fn timeline_probe_runs_real_three_profiles_and_reports_scope() {
    let t = Temp::new();
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(["timeline-probe", t.world().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["dense"]["claim_paid_cents"], 20000);
    assert_eq!(v["joint"]["payroll"][0]["paid"], 112);
    assert_eq!(v["memory_16gb_gate"], "not_run");
    assert_eq!(v["gpu"], "not_run");
    assert_eq!(v["source_sha256"], RULES);
}

#[test]
fn replay_conflict_and_rejected_requests_cannot_reset_release() {
    let t = Temp::new();
    drop(create(&t.world()));
    let out = session(
        &t.world(),
        ["operator", "system"],
        vec![
            request(1, json!({"command":"speed","day_wall_ms":1000})),
            request(2, json!({"command":"resume"})),
            request(3, json!({"command":"run","envelope":env(1,0,3600)})),
            request(4, json!({"command":"run","envelope":env(1,0,3600)})),
            request(5, json!({"command":"run","envelope":env(1,0,3599)})),
            request(6, json!({"command":"resume"})),
            request(7, json!({"command":"speed","day_wall_ms":1000})),
            request(8, json!({"command":"run","envelope":env(2,1,3600)})),
            request(9, json!({"command":"run","envelope":env(3,2,u32::MAX)})),
            request(10, json!({"command":"run","envelope":env(3,2,3600)})),
            request(11, json!({"command":"pause"})),
        ],
    );
    assert_eq!(out[2]["result"]["pacing_wait_ns"], 0);
    assert_eq!(out[3]["result"]["already_recorded"], true);
    assert_eq!(out[3]["result"]["pacing_wait_ns"], 0);
    assert_eq!(
        out[4]["result"]["outcome"]["model"]["Err"],
        "operation_conflict"
    );
    assert_eq!(out[4]["result"]["already_recorded"], false);
    assert_eq!(out[8]["error"]["code"], "wall_budget");
    assert_eq!(out[10]["result"]["committed_game_seconds"], 10800);
    // Assert the actual clock state, not a positive sleep: valid model/IPC
    // time can legitimately exhaust an interval before the next request.
    let deadline = &out[2]["result"]["status"]["next_release_in_session_ns"];
    assert!(deadline.as_u64().unwrap() > 0);
    for i in [3, 4] {
        assert_eq!(
            &out[i]["result"]["status"]["next_release_in_session_ns"],
            deadline
        );
    }
    for i in [5, 6] {
        assert_eq!(&out[i]["result"]["next_release_in_session_ns"], deadline);
    }
    assert!(
        out[7]["result"]["status"]["next_release_in_session_ns"]
            .as_u64()
            .unwrap()
            > deadline.as_u64().unwrap()
    );
    assert!(
        out[9]["result"]["status"]["next_release_in_session_ns"]
            .as_u64()
            .unwrap()
            > out[7]["result"]["status"]["next_release_in_session_ns"]
                .as_u64()
                .unwrap()
    );
    assert!(out[10]["result"]["next_release_in_session_ns"].is_null());
    assert_eq!(
        Store::open(&t.world(), RULES).unwrap().committed_sequence(),
        3
    );
}
